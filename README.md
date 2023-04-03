# silly-alloc

`silly_alloc` is a collection of very basic allocators that are fast and small. Written with WebAssembly in mind.

## Features

- Bump allocators — Fast and small allocators that cannot free memory.
- Bucket allocators — Alloctors that excel at frequent allocations and deallocations of a similar size.
- Works with `#![no_std]`
- Support for and tests on `wasm32-unknown-unknown` and `wasm32-wasi`.

## Warning

This crate is young and experimental. I have tried my best to ensure correct functionality through testing, but it’s very likely that there are bugs. It’s even more likely that features are missing that should be there. Please feel free to open issues or even PRs!

## Examples

- [Examples for bump allocators](bump/index.html)
- [Examples for bucket allocators](bucket/index.html)

## Running the tests

To run the unit and integration tests:

```shell
$ cargo test --target=wasm32-wasi
```

To run the doc tests, Nightly Rust is required (as cross-compiling doc tests is still experimental) and a special environment variable needs to be set so that the macro crate generates the correct absolute paths for the bucket allocator types.

```shell
$ SILLY_ALLOC_DOC_TESTS=1 cargo +nightly test --doc --target wasm32-wasi -Zdoctest-xcompile
```

---
License Apache 2.

License: Apache-2.0
