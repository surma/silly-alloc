use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    fmt::Debug,
    ptr::null_mut,
};

use crate::result::BumpAllocatorMemoryResult;
use crate::{
    head::{Head, SingleThreadedHead},
    result::BumpAllocatorMemoryError,
};

use bytemuck::Zeroable;

pub trait BumpAllocatorMemory {
    fn start(&self) -> *const u8;
    fn size(&self) -> usize;
    fn ensure_min_size(&self, min_size: usize) -> BumpAllocatorMemoryResult<usize>;
    fn past_end(&self, ptr: *const u8) -> Option<usize> {
        // FIXME: This is probably not the best way to handle big
        // memory sizes that overflow isize.
        let v = unsafe {
            self.start()
                .offset(self.size().try_into().ok()?)
                .offset_from(ptr)
        };
        if v < 0 {
            None
        } else {
            v.try_into().ok()
        }
    }
}

pub struct ArenaMemory<const N: usize> {
    arena: [u8; N],
}

unsafe impl<const N: usize> Zeroable for ArenaMemory<N> {}

impl<const N: usize> ArenaMemory<N> {
    pub const fn new() -> Self {
        ArenaMemory { arena: [0u8; N] }
    }
}

impl<const N: usize> BumpAllocatorMemory for ArenaMemory<N> {
    fn start(&self) -> *const u8 {
        self.arena.as_slice().as_ptr()
    }
    fn size(&self) -> usize {
        self.arena.len()
    }
    fn ensure_min_size(&self, _min_size: usize) -> BumpAllocatorMemoryResult<usize> {
        Err(BumpAllocatorMemoryError::GrowthFailed)
    }
}

/// A bump allocator working on memory `M`, tracking where the remaining
/// free memory starts using head `H`.
pub struct BumpAllocator<M: BumpAllocatorMemory, H: Head = SingleThreadedHead> {
    head: UnsafeCell<H>,
    memory: M,
}

// TODO: impl Default for WasmPageMemory + SingleSThreadedHead

impl<M: BumpAllocatorMemory, H: Head + Default> Debug for BumpAllocator<M, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let head: i64 = unsafe { self.head.get().as_ref() }
            .map(|h| h.current() as i64 - self.memory.start() as i64)
            .unwrap_or(-1);
        let size = self.memory.size();
        f.debug_struct("BumpAllocator")
            .field("head", &head)
            .field("size", &size)
            .finish()
    }
}

impl<M: BumpAllocatorMemory, H: Head + Default> BumpAllocator<M, H> {
    pub const fn new(memory: M, head: H) -> Self {
        BumpAllocator {
            memory,
            head: UnsafeCell::new(head),
        }
    }

    pub unsafe fn reset(&self) {
        if let Some(head) = self.try_as_head_mut() {
            head.set(self.memory.start() as usize);
        }
    }

    fn get_head_ptr(&self) -> Option<*const u8> {
        let offset: isize = self.as_head_mut().current().try_into().ok()?;
        unsafe { Some(self.memory.start().offset(offset)) }
    }

    fn try_as_head_mut(&self) -> Option<&mut H> {
        unsafe { self.head.get().as_mut() }
    }

    fn as_head_mut(&self) -> &mut H {
        self.try_as_head_mut().unwrap()
    }
}

unsafe impl<M: BumpAllocatorMemory + Zeroable, H: Head + Zeroable> Zeroable
    for BumpAllocator<M, H>
{
}

unsafe impl<M: BumpAllocatorMemory, H: Head + Default> GlobalAlloc for BumpAllocator<M, H> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();
        let ptr = match self.get_head_ptr() {
            Some(ptr) => ptr,
            _ => return null_mut(),
        };
        let offset = ptr.align_offset(align);
        let head = self.as_head_mut();
        head.add(offset + size);
        if let Some(needed_bytes) = self.memory.past_end(head.current() as *const u8) {
            match self
                .memory
                .ensure_min_size(self.memory.size() + needed_bytes)
            {
                Err(_) => return null_mut(),
                _ => {}
            };
        }

        ptr.offset(offset.try_into().unwrap()) as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{thread_rng, Rng};

    #[test]
    fn minifuzz() {
        const SIZE: usize = 64 * 1024 * 1024;

        unsafe {
            let mut rng = thread_rng();
            static mut ALLOCATOR: BumpAllocator<ArenaMemory<SIZE>> =
                BumpAllocator::new(ArenaMemory::<SIZE>::new(), SingleThreadedHead::new());

            fn reinit() {
                use crate::on_heap::OnHeap;
                type T = BumpAllocator<ArenaMemory<SIZE>>;

                let mut next = T::on_heap();
                unsafe {
                    std::mem::swap(&mut ALLOCATOR, next.as_mut());
                }
            }

            for _attempts in 1..100 {
                if _attempts != 1 {
                    reinit();
                }
                for _allocation in 1..10 {
                    let size = rng.gen_range(1..=32);
                    let alignment = 1 << rng.gen_range(1..=5);
                    let layout = Layout::from_size_align(size, alignment).unwrap();
                    let ptr = ALLOCATOR.alloc(layout) as usize;
                    assert!(ptr % alignment == 0);
                }
            }
        }
    }
}
