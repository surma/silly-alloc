extern crate wasm_alloc_macros;

use wasm_alloc_macros::bucket_allocator;

struct Bucket {
    size: usize,
}
#[test]
fn test1() {
    // assert_eq!(
    //     "size",
    //     bucket_allocator! {
    //         Bucket { size: 4 },
    //         Bucket { size: 5 }
    //     }
    // );
}
