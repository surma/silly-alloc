#![no_std]

#[cfg(test)]
#[macro_use]
extern crate std;

pub(crate) mod head;

pub mod on_heap;
pub mod result;

pub mod bump;
#[cfg(target_arch = "wasm32")]
pub mod wasm_page_bump;
