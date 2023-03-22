use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

use std::alloc::GlobalAlloc;
use wasm_alloc::bucket_allocator;

#[bucket_allocator]
struct BucketAllocator {
    vec2: Bucket<SlotSize<2>, NumSlots<100000>, Align<2>>,
}

fn criterion_benchmark(c: &mut Criterion) {
    let layout = std::alloc::Layout::from_size_align(2, 2).unwrap();
    c.bench_function("Allocate 2 bytes", |b| {
        b.iter_batched(
            || BucketAllocator::new(),
            |a| unsafe { a.alloc(layout) },
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
