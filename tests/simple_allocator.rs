#![no_std]

use wasm_alloc::bucket_allocator;

#[bucket_allocator]
struct BucketAllocator {
    vec2: Bucket<SlotSize<8>, NumSlots<32>, Align<8>>,
    other: Bucket<SlotSize<128>, NumSlots<32>, Align<8>>,
    other2: Bucket<SlotSize<1024>, NumSlots<32>, Align<8>>,
    other3: Bucket<SlotSize<2048>, NumSlots<32>, Align<8>>,
    other4: Bucket<SlotSize<4096>, NumSlots<32>, Align<8>>,
}

#[global_allocator]
static ALLOCATOR: BucketAllocator = BucketAllocator::new();

#[test]
fn test1() {
    // println!("AAAAH");
    // use core::convert::AsMut;
    // let b = Box::new(4).as_mut() as *const _ as usize;
    // assert_eq!(b, ptr1);
}
