#![no_std]

use core::panic::PanicInfo;

use silly_alloc::bump::wasm::WasmMemoryArena;
use silly_alloc::bump::{BumpAllocator, SingleThreadedHead};

extern crate alloc;
use alloc::boxed::Box;

#[global_allocator]
static ALLOCATOR: BumpAllocator<WasmMemoryArena, SingleThreadedHead> =
    BumpAllocator::wasmmemoryarena_singlethreaded();

#[no_mangle]
extern "C" fn test_page_growth() {
    static mut CNT: usize = 0;
    const SIZE: usize = 64 * 1024;
    let num_pages_start = core::arch::wasm32::memory_size::<0>();
    let m: Box<[u8; SIZE]> = Box::new([0u8; SIZE]);
    let r = Box::leak(m);
    unsafe {
        CNT += r as *const u8 as usize;
    }
    let num_pages_end = core::arch::wasm32::memory_size::<0>();
    assert!(num_pages_end > num_pages_start);
}

#[panic_handler]
fn panic_handler(_: &PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
