#![no_std]

//!

pub mod bump;
pub use bump::BumpAllocator;

pub mod bucket;

pub use silly_alloc_macros::bucket_allocator;

// Enable std for testing
#[cfg(test)]
#[macro_use]
extern crate std;
