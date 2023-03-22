use wasm_alloc::{
    bucket::{Bucket, BucketImpl, NumSlots, SlotSize, SlotWithAlign1},
    bucket_allocator,
};

#[bucket_allocator]
struct BucketAllocator {
    vec2: Bucket<SlotSize<8>, NumSlots<32>>,
}

#[global_allocator]
static ALLOCATOR: BucketAllocator = BucketAllocator::new();

#[test]
fn test1() {
    // use core::convert::AsMut;
    // let b = Box::new(4).as_mut() as *const _ as usize;
    // assert_eq!(b, ptr1);
}
