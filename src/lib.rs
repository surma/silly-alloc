#![no_std]

pub mod head;
pub mod result;

pub mod bump;
pub use bump::BumpAllocator;
pub use bump::BumpAllocatorMemory;

pub mod bucket;

pub use wasm_alloc_macros::bucket_allocator;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::WasmPageMemory;

#[cfg(test)]
#[macro_use]
extern crate std;

#[cfg(test)]
mod e2e_tests;
