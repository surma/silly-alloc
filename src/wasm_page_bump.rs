// LLVM will set the address of `__heap_base` to the start of the heap area.
extern "C" {
    static __heap_base: u8;
}

use crate::result::BumpAllocatorMemoryResult;
use crate::{bump::BumpAllocatorMemory, result::BumpAllocatorMemoryError};

pub struct WasmPageMemory;

const PAGE_SIZE: usize = 64 * 1024;
impl BumpAllocatorMemory for WasmPageMemory {
    fn start(&self) -> *const u8 {
        &__heap_base
    }

    fn size(&self) -> usize {
        core::arch::wasm32::memory_size(0) * PAGE_SIZE - self.memory.start() as usize
    }

    fn ensure_min_size(&self, min_size: usize) -> BumpAllocatorMemoryResult<usize> {
        let total_mem_size = min_size + self.start() as usize;
        let delta_pages_f = (total_mem_size as f32) / (PAGE_SIZE as f32);
        let mut delta_pages: usize = delta_pages_f as usize;
        if delta_pages_f % 1.0 != 0.0 {
            delta_pages += 1;
        }
        let new_num_pages = core::arch::wasm32::memory_grow(0, delta_pages);
        if new_num_pages == usize::MAX {
            return Err(BumpAllocatorMemoryError::GrowthFailed);
        }
        Ok(new_num_pages * PAGE_SIZE - self.start() as usize)
    }
}
