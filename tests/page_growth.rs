#![no_std]

#[cfg(target_family = "wasm")]
mod wasmtest {
    use silly_alloc::WasmBumpAllocator;

    extern crate alloc;
    use alloc::vec::Vec;

    #[global_allocator]
    static ALLOCATOR: WasmBumpAllocator = WasmBumpAllocator::with_memory();

    #[test]
    fn test_page_growth() {
        let num_pages_start = core::arch::wasm32::memory_size::<0>();
        let size = ALLOCATOR.arena().size();
        let _v: Vec<u8> = Vec::with_capacity(size);
        let num_pages_end = core::arch::wasm32::memory_size::<0>();
        assert!(num_pages_end > num_pages_start);
    }
}
