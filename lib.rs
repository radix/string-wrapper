//! provides `StringWrapper`, most useful for stack-based strings.
#![deny(missing_docs)]

#[cfg(feature="use_serde")]
#[macro_use]
extern crate serde_derive;
#[cfg(feature="use_serde")]
extern crate serde;

use std::borrow;
use std::fmt;
use std::io::Write;
use std::mem::transmute;
use std::ops;
use std::ptr;
use std::str;
use std::cmp;
use std::hash;

#[cfg(feature="use_serde")]
use serde::de::Error;

/// Like `String`, but with a fixed capacity and a generic backing bytes storage.
///
/// Use e.g. `StringWrapper<[u8; 4]>` to have a string without heap memory allocation.
#[derive(Copy, Default)]
pub struct StringWrapper<T>
    where T: Buffer
{
    len: usize,
    buffer: T,
}

/// Equivalent to `AsMut<[u8]> + AsRef<[u8]>` with the additional constraint that
/// implementations must return the same slice from subsequent calls of `as_mut` and/or `as_ref`.
pub unsafe trait Buffer {
    /// Get the backing buffer as a slice.
    fn as_ref(&self) -> &[u8];
    /// Get the backing buffer as a mutable slice.
    fn as_mut(&mut self) -> &mut [u8];
}

/// The OwnedBuffer trait is in support of StringWrapper::from_str, since we need to be able to
/// allocate new buffers for it.
///
/// IMPLEMENTATION NOTE: There is currently no impl for Vec<u8>, because StringWrapper assumes a
/// fixed capacity, and we don't have a way to know what size vec we should return.
// Besides, I'm not sure what the value of Buffer for Vec is anyway, when you could just use
// String...
pub trait OwnedBuffer: Buffer {
    /// Creature a new buffer that can be used to initialize a StringWrapper.
    fn new() -> Self;
}

impl<T> StringWrapper<T>
    where T: Buffer
{
    /// Create an empty string from its backing storage.
    pub fn new(buffer: T) -> Self {
        StringWrapper {
            len: 0,
            buffer: buffer,
        }
    }

    /// Unsafely create a string from its components.
    ///
    /// Users must ensure that:
    ///
    /// * The buffer length is at least `len`
    /// * The first `len` bytes of `buffer` are well-formed UTF-8.
    pub unsafe fn from_raw_parts(buffer: T, len: usize) -> Self {
        StringWrapper {
            len: len,
            buffer: buffer,
        }
    }

    /// Consume the string and return the backing storage.
    pub fn into_buffer(self) -> T {
        self.buffer
    }

    /// View the backing storage as a bytes slice.
    pub fn buffer(&self) -> &[u8] {
        self.buffer.as_ref()
    }


    /// View the backing storage as a bytes slice.
    ///
    /// Users must ensure that the prefix bytes up to `self.len()` remains well-formed UTF-8.
    pub unsafe fn buffer_mut(&mut self) -> &mut [u8] {
        self.buffer.as_mut()
    }

    /// Return the number of bytes in the string.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the string contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Unsafely change the length in bytes of the string.
    ///
    /// Users must ensure that the string remains well-formed UTF-8.
    pub unsafe fn set_len(&mut self, new_len: usize) {
        self.len = new_len
    }

    /// Shortens a string to the specified length.
    ///
    /// Panics if `new_len` > current length, or if `new_len` is not a character boundary.
    pub fn truncate(&mut self, new_len: usize) {
        assert!(new_len <= self.len);
        if new_len < self.len {
            assert!(starts_well_formed_utf8_sequence(self.buffer.as_ref()[new_len]));
        }
        self.len = new_len;
    }

    /// Return the maximum number of bytes the string can hold.
    pub fn capacity(&self) -> usize {
        self.buffer.as_ref().len()
    }

    /// Return by how many bytes the string can grow.
    pub fn extra_capacity(&self) -> usize {
        self.capacity() - self.len
    }

    /// Return the slice of unused bytes after the string
    pub fn extra_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.buffer.as_mut()[self.len..]
    }

    /// Append a code point to the string if the extra capacity is sufficient.
    ///
    /// Return `Ok` with the code point appended, or `Err` with the string unchanged.
    pub fn push(&mut self, c: char) -> Result<(), ()> {
        let new_len = self.len + c.len_utf8();
        if new_len <= self.capacity() {
            // FIXME: use `c.encode_utf8` once it’s stable.
            write!(self.extra_bytes_mut(), "{}", c).unwrap();
            self.len = new_len;
            Ok(())
        } else {
            Err(())
        }
    }

    /// Append a string slice to the string.
    ///
    /// Panics if the extra capacity is not sufficient.
    pub fn push_str(&mut self, s: &str) {
        copy_memory(s.as_bytes(), self.extra_bytes_mut());
        self.len += s.len();
    }

    /// Append as much as possible of a string slice to the string.
    ///
    /// Return `Ok(())` if the extra capacity was sufficient,
    /// or `Err(n)` where `n` is the number of bytes pushed.
    /// `n` is within 3 bytes of the extra capacity.
    pub fn push_partial_str(&mut self, s: &str) -> Result<(), usize> {
        let mut i = self.extra_capacity();
        let (s, result) = if i < s.len() {
            // As long as `self` is well-formed,
            // this loop does as most 3 iterations and `i` does not underflow.
            while !starts_well_formed_utf8_sequence(s.as_bytes()[i]) {
                i -= 1
            }
            (&s[..i], Err(i))
        } else {
            (s, Ok(()))
        };
        self.push_str(s);
        result
    }
}

impl<T: OwnedBuffer> StringWrapper<T> {
    /// Copy a `&str` into a new `StringWrapper`. You may need to annotate the type of this call so
    /// Rust knows which size of array you want to populate:
    ///
    /// # Panics
    ///
    /// Panics if the `&str` cannot fit into the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use string_wrapper::StringWrapper;
    ///
    /// let sw: StringWrapper<[u8; 32]> = StringWrapper::from_str("hello, world");
    /// assert_eq!(format!("{}", sw), "hello, world");
    /// ```
    pub fn from_str(s: &str) -> StringWrapper<T> {
        let buffer = T::new();
        let mut sw = StringWrapper::new(buffer);
        sw.push_str(s);
        sw
    }

    /// Safely construct a new StringWrapper from a &str. Unlike `from_str`, this method doesn't
    /// panic when the &str is too big to fit into the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use string_wrapper::StringWrapper;
    ///
    /// let sw: Option<StringWrapper<[u8; 3]>> = StringWrapper::from_str_safe("foo");
    /// assert_eq!(format!("{}", sw.unwrap()), "foo");
    /// ```
    ///
    /// ```
    /// use string_wrapper::StringWrapper;
    ///
    /// let sw: Option<StringWrapper<[u8; 3]>> = StringWrapper::from_str_safe("foobar");
    /// assert_eq!(sw, None);
    /// ```
    pub fn from_str_safe(s: &str) -> Option<StringWrapper<T>> {
        let buffer = T::new();
        let mut sw = StringWrapper::new(buffer);
        match sw.push_partial_str(s) {
            Ok(_) => Some(sw),
            Err(_) => None,
        }
    }
}

fn starts_well_formed_utf8_sequence(byte: u8) -> bool {
    // ASCII byte or "leading" byte
    byte < 128 || byte >= 192
}

// FIXME: Use `std::slice::bytes::copy_memory` instead when it’s stable.
/// Copies data from `src` to `dst`
///
/// Panics if the length of `dst` is less than the length of `src`.
fn copy_memory(src: &[u8], dst: &mut [u8]) {
    let len_src = src.len();
    assert!(dst.len() >= len_src);
    // `dst` is unaliasable, so we know statically it doesn't overlap
    // with `src`.
    unsafe {
        ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), len_src);
    }
}

impl<T> ops::Deref for StringWrapper<T>
    where T: Buffer
{
    type Target = str;

    fn deref(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.buffer.as_ref()[..self.len]) }
    }
}

impl<T> ops::DerefMut for StringWrapper<T>
    where T: Buffer
{
    fn deref_mut(&mut self) -> &mut str {
        unsafe { transmute::<&mut [u8], &mut str>(&mut self.buffer.as_mut()[..self.len]) }
    }
}

impl<T> borrow::Borrow<str> for StringWrapper<T>
    where T: Buffer
{
    fn borrow(&self) -> &str {
        self
    }
}

impl<T> fmt::Display for StringWrapper<T>
    where T: Buffer
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T> fmt::Debug for StringWrapper<T>
    where T: Buffer
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: Buffer> PartialEq for StringWrapper<T> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

// We need to explicitly define Eq here, because the derive logic only impls it when T is also Eq.
impl<T: Buffer> Eq for StringWrapper<T> {}

// Likewise we need to implement Clone explicitly because std doesn't define it for arrays bigger
// than 32 elements. We rely on cloning the slice of the array and then copying that into a new
// buffer, which requires OwnedBuffer::new.
impl<T: Buffer + Copy> Clone for StringWrapper<T> {
    fn clone(&self) -> Self {
        *self

    }
}

impl<T: Buffer> PartialOrd for StringWrapper<T> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Buffer> hash::Hash for StringWrapper<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        (**self).hash(state)
    }
}

impl<T: Buffer> Ord for StringWrapper<T> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        (**self).cmp(&**other)
    }
}

#[cfg(feature="use_serde")]
impl<T: Buffer> serde::Serialize for StringWrapper<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_string().serialize(serializer)
    }
}

#[cfg(feature="use_serde")]
impl<'de, T: OwnedBuffer> serde::Deserialize<'de> for StringWrapper<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s: String = serde::Deserialize::deserialize(deserializer)?;
        let sb = StringWrapper::from_str_safe(&s).ok_or_else(|| {
                let buff = T::new();
                let msg: String = format!("string that can fit into {} bytes", buff.as_ref().len());
                D::Error::invalid_length(s.len(), &StringExpected(msg))
            })?;
        Ok(sb)
    }
}

// It seems silly that I can't just pass a String to invalid_length, but there's no implementation
// of Expected for String, so...
#[cfg(feature="use_serde")]
struct StringExpected(String);
#[cfg(feature="use_serde")]
impl serde::de::Expected for StringExpected {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

unsafe impl<'a, T: ?Sized + Buffer> Buffer for &'a mut T {
    fn as_ref(&self) -> &[u8] {
        (**self).as_ref()
    }
    fn as_mut(&mut self) -> &mut [u8] {
        (**self).as_mut()
    }
}

unsafe impl<'a, T: ?Sized + Buffer> Buffer for Box<T> {
    fn as_ref(&self) -> &[u8] {
        (**self).as_ref()
    }
    fn as_mut(&mut self) -> &mut [u8] {
        (**self).as_mut()
    }
}

unsafe impl Buffer for Vec<u8> {
    fn as_ref(&self) -> &[u8] {
        self
    }
    fn as_mut(&mut self) -> &mut [u8] {
        self
    }
}

unsafe impl Buffer for [u8] {
    fn as_ref(&self) -> &[u8] {
        self
    }
    fn as_mut(&mut self) -> &mut [u8] {
        self
    }
}

macro_rules! array_impl {
    ($($N: expr)+) => {
        $(
            unsafe impl Buffer for [u8; $N] {
                fn as_ref(&self) -> &[u8] { self }
                fn as_mut(&mut self) -> &mut [u8] { self }
            }

            impl OwnedBuffer for [u8; $N] {
                fn new() -> Self { [0u8; $N] }
            }
        )+
    }
}

array_impl! {
     0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
    64 128 256 512 1024
    2 * 1024
    4 * 1024
    8 * 1024
    16 * 1024
    32 * 1024
    64 * 1024
    128 * 1024
    256 * 1024
    512 * 1024
    1024 * 1024
    2 * 1024 * 1024
    4 * 1024 * 1024
    8 * 1024 * 1024
    16 * 1024 * 1024
    32 * 1024 * 1024
    64 * 1024 * 1024
    128 * 1024 * 1024
    256 * 1024 * 1024
    512 * 1024 * 1024
    1024 * 1024 * 1024
    100 1_000 10_000 100_000 1_000_000
    10_000_000 100_000_000 1_000_000_000
}

#[cfg(test)]
mod tests {
    use std;
    use std::cmp;
    use std::hash;

    #[cfg(feature="use_serde")]
    extern crate serde_json;

    use StringWrapper;

    #[test]
    fn traits() {
        // A simple way to ensure that Eq is implemented for StringWrapper
        #[derive(Eq, PartialEq, Ord, PartialOrd)]
        struct Foo {
            x: StringWrapper<[u8; 64]>,
        }
    }

    #[test]
    fn eq() {
        let mut s = StringWrapper::<[u8; 3]>::new(*b"000");
        assert_eq!(s, s);
        s.push_str("foo");
        let mut s2 = StringWrapper::<[u8; 3]>::new(*b"000");
        s2.push_str("foo");
        assert_eq!(s, s2);

        let mut s3 = StringWrapper::<[u8; 3]>::new(*b"000");
        s3.push_str("bar");
        assert!(s != s3);
    }

    #[test]
    fn eq_only_to_length() {
        let a = StringWrapper::<[u8; 5]>::new(*b"aaaaa");
        let b = StringWrapper::<[u8; 5]>::new(*b"bbbbb");
        assert_eq!(a, b);
    }

    #[test]
    fn ord() {
        let mut s = StringWrapper::<[u8; 3]>::new(*b"000");
        let mut s2 = StringWrapper::<[u8; 3]>::new(*b"000");
        s.push_str("a");
        s2.push_str("b");
        assert_eq!(s.partial_cmp(&s2), Some(cmp::Ordering::Less));
        assert_eq!(s.cmp(&s2), cmp::Ordering::Less);
    }

    #[test]
    fn ord_only_to_length() {
        let mut s = StringWrapper::<[u8; 3]>::new(*b"000");
        let mut s2 = StringWrapper::<[u8; 3]>::new(*b"111");
        assert_eq!(s.partial_cmp(&s2), Some(cmp::Ordering::Equal));
        assert_eq!(s.cmp(&s2), cmp::Ordering::Equal);

        s.push_str("aa");
        s2.push_str("aa");
        assert_eq!(s.partial_cmp(&s2), Some(cmp::Ordering::Equal));
        assert_eq!(s.cmp(&s2), cmp::Ordering::Equal);
    }

    #[cfg(test)]
    fn hash<T: hash::Hash>(t: &T) -> u64 {
        // who knows why this isn't in std
        let mut h = std::collections::hash_map::DefaultHasher::new();
        t.hash(&mut h);
        hash::Hasher::finish(&h)
    }

    #[test]
    fn hash_only_to_length() {
        let mut s = StringWrapper::<[u8; 64]>::new([0u8; 64]);
        let mut s2 = StringWrapper::<[u8; 64]>::new([1u8; 64]);
        assert_eq!(hash(&s), hash(&s2));
        s.push_str("a");
        assert!(hash(&s) != hash(&s2));
        s2.push_str("a");
        assert_eq!(hash(&s), hash(&s2));
    }

    #[test]
    fn from_str() {
        let s: StringWrapper<[u8; 64]> = StringWrapper::from_str("OMG!");
        let mut s2 = StringWrapper::new([0u8; 64]);
        s2.push_str("OMG!");
        assert_eq!(s, s2);
    }

    #[test]
    fn it_works() {
        let mut s = StringWrapper::new([0; 10]);
        assert_eq!(&*s, "");
        assert_eq!(s.len(), 0);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 10);

        assert_eq!(&*s, "");
        assert_eq!(s.len(), 0);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 10);

        s.push_str("a");
        assert_eq!(&*s, "a");
        assert_eq!(s.len(), 1);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 9);

        assert_eq!(s.push('é'), Ok(()));
        assert_eq!(&*s, "aé");
        assert_eq!(s.len(), 3);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 7);

        assert_eq!(s.push_partial_str("~~~"), Ok(()));
        assert_eq!(&*s, "aé~~~");
        assert_eq!(s.len(), 6);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 4);

        assert_eq!(s.push_partial_str("hello"), Err(4));
        assert_eq!(&*s, "aé~~~hell");
        assert_eq!(s.len(), 10);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 0);

        s.truncate(6);
        assert_eq!(&*s, "aé~~~");
        assert_eq!(s.len(), 6);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 4);

        assert_eq!(s.push_partial_str("_🌠"), Err(1));
        assert_eq!(&*s, "aé~~~_");
        assert_eq!(s.len(), 7);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 3);

        assert_eq!(s.push('🌠'), Err(()));
        assert_eq!(&*s, "aé~~~_");
        assert_eq!(s.len(), 7);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 3);


        let buffer: [u8; 10] = s.clone().into_buffer();
        assert_eq!(&buffer, b"a\xC3\xA9~~~_ell");
        assert_eq!(format!("{}", s), "aé~~~_");
        assert_eq!(format!("{:?}", s), r#""aé~~~_""#);

        assert_eq!(s.push_partial_str("ô!?"), Err(3));
        assert_eq!(&*s, "aé~~~_ô!");
        assert_eq!(s.len(), 10);
        assert_eq!(s.capacity(), 10);
        assert_eq!(s.extra_capacity(), 0);
    }

    #[cfg(feature="use_serde")]
    #[test]
    fn test_serde() {
        let mut s = StringWrapper::new([0u8; 64]);
        s.push_str("foobar");
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"foobar\"");
        let s2 = serde_json::from_str(&json).unwrap();
        assert_eq!(s, s2);
    }

    #[cfg(feature="use_serde")]
    #[test]
    fn deserialize_too_long() {
        let json = "\"12345\"";
        match serde_json::from_str::<StringWrapper<[u8; 3]>>(&json) {
            Err(e) => {
                assert_eq!(format!("{}", e),
                           "invalid length 5, expected string that can fit into 3 bytes")
            }
            Ok(x) => panic!("Expected error, got success: {:?}", x),
        }
    }

    #[test]
    fn clone() {
        let s = StringWrapper::new([0u8; 64]);
        let y: StringWrapper<[u8; 64]> = s.clone();
        println!("s: {}, y: {}", s, y);
    }
}
