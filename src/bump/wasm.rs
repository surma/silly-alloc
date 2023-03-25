use core::{cell::UnsafeCell, marker::PhantomData};

pub use crate::bump::{
    head::{Head, SingleThreadedHead, ThreadSafeHead},
    BumpAllocator, BumpAllocatorArena, BumpAllocatorArenaError, BumpAllocatorArenaResult,
};

// LLVM will set the address of `__heap_base` to the start of the heap area.
extern "C" {
    static __heap_base: u8;
}

/// A [`BumpAllocatorArena`] implementation that works on the entire WebAssembly memory. The generic `N` determines which memory to use, making this allocator ready for the [multi-memory proposal].
///
/// The `WasmMemoryArena` uses the LLVM `__heap_base` symbol that is provided by the linker as the starting value for the head.
/// [multi-memory proposal]: https://lol.com
pub struct WasmMemoryArena<const N: u32 = 0>();

impl<const N: u32> WasmMemoryArena<N> {
    pub const fn new() -> Self {
        WasmMemoryArena {}
    }
}

impl<const N: u32> Default for WasmMemoryArena<N> {
    fn default() -> Self {
        Self::new()
    }
}

const PAGE_SIZE: usize = 64 * 1024;
impl<const N: u32> BumpAllocatorArena for WasmMemoryArena<N> {
    fn start(&self) -> *const u8 {
        unsafe { &__heap_base }
    }

    fn size(&self) -> usize {
        core::arch::wasm32::memory_size(N) * PAGE_SIZE - self.start() as usize
    }

    fn ensure_min_size(&self, min_size: usize) -> BumpAllocatorArenaResult<usize> {
        let total_mem_size = min_size + self.start() as usize;
        let delta_pages_f = (total_mem_size as f32) / (PAGE_SIZE as f32);
        let mut delta_pages: usize = delta_pages_f as usize;
        if delta_pages_f % 1.0 != 0.0 {
            delta_pages += 1;
        }
        let new_num_pages = core::arch::wasm32::memory_grow(N, delta_pages);
        if new_num_pages == usize::MAX {
            return Err(BumpAllocatorArenaError::GrowthFailed);
        }
        Ok(new_num_pages * PAGE_SIZE - self.start() as usize)
    }
}

/// A `BumpAllocator` that uses the entire Wasm memory as the arena.
pub type WasmBumpAllocator = BumpAllocator<'static, WasmMemoryArena<0>, SingleThreadedHead>;

impl WasmBumpAllocator {
    pub const fn with_memory() -> WasmBumpAllocator {
        BumpAllocator {
            memory: WasmMemoryArena::new(),
            head: UnsafeCell::new(SingleThreadedHead::new()),
            lifetime: PhantomData,
        }
    }
}

/// A `BumpAllocator` that uses the entire Wasm memory as the arena and can be used for multithreaded WebAssembly modules.
pub type ThreadsafeWasmBumpAllocator = BumpAllocator<'static, WasmMemoryArena<0>, ThreadSafeHead>;

impl ThreadsafeWasmBumpAllocator {
    pub const fn with_memory() -> ThreadsafeWasmBumpAllocator {
        BumpAllocator {
            memory: WasmMemoryArena::new(),
            head: UnsafeCell::new(ThreadSafeHead::new()),
            lifetime: PhantomData,
        }
    }
}
