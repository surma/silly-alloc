#![no_std]

use wasm_alloc::bucket_allocator;

extern crate alloc;
use alloc::boxed::Box;

#[bucket_allocator]
struct BucketAllocator {
    vec2: Bucket<SlotSize<4>, NumSlots<32>, Align<4>>,
    // Massive and extremely wasteful overflow bucket for test runtime
    other: Bucket<SlotSize<65536>, NumSlots<1024>, Align<512>>,
}

#[global_allocator]
static ALLOCATOR: BucketAllocator = BucketAllocator::new();

#[test]
fn test1() {
    let b1 = Box::new(4);
    let ptr1 = b1.as_ref() as *const i32 as usize;
    let b2 = Box::new(4);
    drop(b1);
    let b3 = Box::new(4);
    let ptr3 = b3.as_ref() as *const i32 as usize;
    drop(b2);
    drop(b3);
    assert_eq!(ptr1, ptr3);
}
