# Version 0.1.7 (2017-01-19)

## `from_str_safe` method

A version of `from_str` that returns a `Result<StringWrapper<T>,
StringWrapperError>`, so we can avoid panics when constructing StringWrappers.

The Serde `Deserialize` implementation now uses `from_str_safe` so panics will
no longer occur for serialized input that is too long.

# Version 0.1.6 (2017-01-13)

## `from_str` method

`StringWrapper<T>` now has a `from_str(&str) -> StringWrapper<T>` method when
the buffer type `T` is a fixed-sized array.

## Serde support

With the `use_serde` flag, `StringWrapper<T>` will implement the `Serialize` and
`Deserialize` traits from Serde.

## New basic traits

These new traits have been implemented for `StringWrapper<T>`:

- PartialEq, Eq
- PartialOrd, Ord
- Hash

These traits should even work when the underlying sized array types do not
implement them. For example, while `[u8; 64]` does not implement `Eq`,
`StringWrapper<[u8; 64]>` does.
