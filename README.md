[![Build Status](https://api.travis-ci.org/radix/string-wrapper.svg?branch=master)](https://travis-ci.org/radix/string-wrapper) [![Latest Version](https://img.shields.io/crates/v/string-wrapper.svg)](https://crates.io/crates/string-wrapper)

# string_wrapper

string_wrapper is a crate which provides StringWrapper, which is a usually*
stack-allocated UTF-8 string type. Features:

- Array-backed StringWrappers can be entirely stored on the stack
- The Copy trait can be implemented, unlike for standard Strings
- Serde Serialization and Deserialization traits are implemented to act exactly
  like String

# Documentation

Docs are at http://docs.rs/string-wrapper

# Example

First, add this to your `Cargo.toml`:

```toml
[dependencies]
string-wrapper = "0.2"
```

If you want to use [Serde](https://serde.rs/) support, you have to enable the
`use_serde` feature and use Rust 1.15 or higher.

```toml
[dependencies]
string-wrapper = {version = "0.1.6", features = ["use_serde"]}
```

Make sure to use `extern crate` in your "crate root" module (usually either
`lib.rs` or `main.rs`)

```rust
extern crate string_wrapper;
```

Finally, to actually use the StringWrapper type:

```rust
use string_wrapper::StringWrapper;

fn foo() {
  // `from_str` may panic; use `from_str_safe` if you're using arbitrary input
  let s: StringWrapper<[u8; 32]> = StringWrapper::from_str("foo");

  // a StringWrapper can be converted back to a String with `to_string`:
  println!("{}", s.to_string());
  // However, it also supports the Display trait directly:
  println!("{}", s);
}
```

Note that the type parameter MUST be made up of `u8`s, usually* as a `[u8; N]`
array. Possible array sizes for arrays are listed in the
`Implementors` section of the `Buffer` trait documentation:
https://docs.rs/string-wrapper/*/string_wrapper/trait.Buffer.html.

Many other traits are supported by StringWrapper. See the
[http://docs.rs/string-wrapper/](docs).

# "Usually*"? Heap-allocated StringWrappers

`Vec<u8>` is also supported as a backing buffer instead of `[u8; N]`. Using a
`Vec<u8>` means your string will be on the heap.

# When is it useful?

This can be useful if you have tons of small strings that fit within a fixed
length, and the overhead of dealing with pointers to those small strings is
detrimental to your programs. If you're unsure, you should probably just use
String since it's more flexible and convenient.

# Is this SSO (Small-String Optimization)?

Note that this is not what is typically called "SSO String", which is a
dynamically-sized string that is either stored directly on the stack (if it's
small) or on the heap (if it's large). Such a string would not be able to
implement the Copy trait.

# Credits

Thanks to [@SimonSapin](https://github.com/SimonSapin/), the original author of
this code.

Also:

- [@mbrubeck](https://github.com/mbrubeck/)

# LICENSE

string-wrapper is dual-licensed under the [MIT
license](https://opensource.org/licenses/MIT) and the [Apache 2.0
license](https://opensource.org/licenses/Apache-2.0). All contributions must be
made under the terms of both of these licenses.
