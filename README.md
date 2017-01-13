# string_wrapper, a stack-allocated fixed-length string type

string_wrapper is a crate which provides a type, StringWrapper, which is a
stack-allocated UTF-8 string. This has a few consequences:

- Strings must be fixed-length so the size of the type can be known at
  compile-time
- It can implement Clone, unlike the standard heap-allocated String type.

# Documentation

Docs are at http://docs.rs/string-wrapper

# Example

First, add this to your `Cargo.toml`

```toml
[dependencies]
string-wrapper = "0.1.6"
```

Make sure to use `extern crate` at your "crate root" module (usually either
`lib.rs` or `main.rs`)

```rust
extern crate string_wrapper;
```

Finally, to actually use the StringWrapper type:

```rust
use string_wrapper::StringWrapper;

fn foo() {
  // s is of type StringWrapper<[u8; 32]>
  let mut s = StringWrapper::new([0u8; 32]);
  s.push_str("foo");

  // a StringWrapper can be converted back to a String with `to_string`:
  println!("{}", s.to_string());
  // However, it also supports the Display trait directly:
  println!("{}", s);
}
```

Note that the type parameter MUST be a `[u8; N]` array. Possible sizes for `N`
are listed here:

Many other traits are supported by StringWrapper: see the
[http://docs.rs/string-wrapper/](docs).

# When is it useful?

This can be useful if you have tons of small strings that fit within a fixed
length, and the overhead of dealing with pointers to those small strings is
detrimental to your programs. If you're unsure, you should probably just use
String since it's more flexible and convenient.

# Is this SSO (Small-String Optimization)?

Note that this is not what is typically called "SSO String", which is a
dynamically-sized string that is either stored directly on the stack (if it's
small) or on the heap (if it's long). Such a string would not be able to
implement the Copy trait.
