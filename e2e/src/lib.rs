use wasm_alloc::wasm_page_bump::WasmPageBumpAllocator;

#[global_allocator]
static ALLOCATOR: WasmPageBumpAllocator = WasmPageBumpAllocator::new();

static mut CNT: usize = 0;
#[no_mangle]
extern "C" fn _start() {
    for _ in 1..10 {
        let m: Box<[u8; 1 * 1024 * 1024]> = Box::new([0u8; 1 * 1024 * 1024]);
        let r = Box::leak(m);
        unsafe {
            CNT += r as *const u8 as usize;
        }
    }
}
