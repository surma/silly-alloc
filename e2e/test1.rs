extern crate wasm_alloc;
use wasm_alloc::wasm_page_bump::WasmPageBumpAllocator;

#[global_allocator]
static ALLOCATOR: WasmPageBumpAllocator = WasmPageBumpAllocator::new();

#[no_mangle]
extern "C" fn _start() -> i32 {
    let mut s: String = String::new();
    s += "123";
    s += "456";
    s.len() as i32
}
