#![no_std]

#[cfg(all(target_family = "wasm", feature = "atomics"))]
mod wasmtest {
    use silly_alloc::bump::ThreadsafeWasmBumpAllocator;

    extern crate alloc;
    use alloc::vec::Vec;

    #[global_allocator]
    static ALLOCATOR: ThreadsafeWasmBumpAllocator = ThreadsafeWasmBumpAllocator::with_memory();

    #[test]
    fn test_page_growth_threaded() {
        let num_pages_start = core::arch::wasm32::memory_size::<0>();
        let size = ALLOCATOR.arena().size();
        let _v: Vec<u8> = Vec::with_capacity(size);
        let num_pages_end = core::arch::wasm32::memory_size::<0>();
        assert!(num_pages_end > num_pages_start);
    }
}
