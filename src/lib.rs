#![no_std]

/*!

`silly_alloc` is a collection of very basic allocators that are fast and small, specifically with WebAssembly in mind.

# Features

- Bump allocators — Fast and small allocators that cannot free memory
- Bucket allocators — Alloctors that excel at frequent allocations of a similar size
- `#![no_std]`
- Support for and tests on `wasm32-unknown-unknown`

# Examples

- [Examples for bump allocators](bump/index.html)
- [Examples for bucket allocators](bucket/index.html)
*/

pub mod bump;
pub use bump::BumpAllocator;
pub use bump::SliceBumpAllocator;
#[cfg(target_arch = "wasm32")]
pub use bump::WasmBumpAllocator;

pub mod bucket;

pub use silly_alloc_macros::bucket_allocator;

// Enable std for testing
#[cfg(test)]
#[macro_use]
extern crate std;
