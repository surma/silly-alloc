use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    fmt::Debug,
    marker::PhantomData,
    ptr::null_mut,
};

use crate::{head::ThreadSafeHead, result::BumpAllocatorMemoryResult};
use crate::{
    head::{Head, SingleThreadedHead},
    result::BumpAllocatorMemoryError,
};

pub trait BumpAllocatorMemory {
    fn start(&self) -> *const u8;
    fn size(&self) -> usize;
    fn ensure_min_size(&self, min_size: usize) -> BumpAllocatorMemoryResult<usize>;
    fn past_end(&self, ptr: *const u8) -> Option<usize> {
        // FIXME: This is probably not the best way to handle big
        // memory sizes that overflow isize.
        let v = unsafe {
            self.start()
                .offset(self.size().try_into().unwrap())
                .offset_from(ptr)
        };
        if v > 0 {
            None
        } else {
            // Panic, if we can’t convernt the `isize` to an `uszie`.
            Some(v.abs().try_into().unwrap())
        }
    }
}

impl BumpAllocatorMemory for &mut [u8] {
    fn start(&self) -> *const u8 {
        self.as_ptr()
    }
    fn size(&self) -> usize {
        self.len()
    }
    fn ensure_min_size(&self, _min_size: usize) -> BumpAllocatorMemoryResult<usize> {
        Err(BumpAllocatorMemoryError::GrowthFailed)
    }
}

/// A bump allocator working on memory `M`, tracking where the remaining
/// free memory starts using head `H`.
pub struct BumpAllocator<'a, M: BumpAllocatorMemory = &'a mut [u8], H: Head = SingleThreadedHead> {
    head: UnsafeCell<H>,
    memory: M,
    lifetime: PhantomData<&'a u8>,
}

// TODO: impl Default for WasmPageMemory + SingleSThreadedHead

impl<'a, M: BumpAllocatorMemory, H: Head + Default> Debug for BumpAllocator<'a, M, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let head: i64 = unsafe { self.head.get().as_ref() }
            .unwrap()
            .num_bytes_used() as i64;
        let size = self.memory.size();
        f.debug_struct("BumpAllocator")
            .field("head", &head)
            .field("size", &size)
            .finish()
    }
}

impl<'a> BumpAllocator<'a, &'a mut [u8], ThreadSafeHead> {
    pub fn default_threadsafe(arena: &'a mut [u8]) -> Self {
        Self::new(arena, ThreadSafeHead::new())
    }
}

impl<'a> BumpAllocator<'a, &'a mut [u8], SingleThreadedHead> {
    pub fn default_single_threaded(arena: &'a mut [u8]) -> Self {
        Self::new(arena, SingleThreadedHead::new())
    }
}

impl<'a, M: BumpAllocatorMemory, H: Head + Default> BumpAllocator<'a, M, H> {
    pub const fn new(memory: M, head: H) -> Self {
        BumpAllocator {
            memory,
            head: UnsafeCell::new(head),
            lifetime: PhantomData,
        }
    }

    pub unsafe fn reset(&self) {
        if let Some(head) = self.try_as_head_mut() {
            head.set(self.memory.start() as usize);
        }
    }

    fn get_head_ptr(&self) -> Option<*const u8> {
        let offset: isize = self.as_head_mut().num_bytes_used().try_into().ok()?;
        unsafe { Some(self.memory.start().offset(offset)) }
    }

    fn try_as_head_mut(&self) -> Option<&mut H> {
        unsafe { self.head.get().as_mut() }
    }

    fn as_head_mut(&self) -> &mut H {
        self.try_as_head_mut().unwrap()
    }
}

unsafe impl<'a, M: BumpAllocatorMemory, H: Head> Sync for BumpAllocator<'a, M, H> {}

unsafe impl<'a, M: BumpAllocatorMemory, H: Head + Default> GlobalAlloc for BumpAllocator<'a, M, H> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();
        let ptr = match self.get_head_ptr() {
            Some(ptr) => ptr,
            _ => return null_mut(),
        };
        let offset = ptr.align_offset(align);
        let head = self.as_head_mut();
        let last_byte_of_new_allocation = self
            .memory
            .start()
            .offset((head.num_bytes_used() + offset + size).try_into().unwrap())
            .offset(-1);
        if let Some(needed_bytes) = self.memory.past_end(last_byte_of_new_allocation) {
            match self
                .memory
                .ensure_min_size(self.memory.size() + needed_bytes)
            {
                Err(_) => return null_mut(),
                _ => {}
            };
        }

        head.bump(offset + size);
        ptr.offset(offset.try_into().unwrap()) as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{thread_rng, Rng};
    use std::vec::Vec;

    #[test]
    fn increment() {
        let mut arena = [0u8; 1024];
        {
            let allocator = BumpAllocator::default_single_threaded(arena.as_mut_slice());
            unsafe {
                let ptr1 = allocator.alloc(Layout::from_size_align(3, 4).unwrap()) as usize;
                assert!(ptr1 % 4 == 0);
                let ptr2 = allocator.alloc(Layout::from_size_align(3, 4).unwrap()) as usize;
                assert!(ptr2 % 4 == 0);
                assert!(
                    ptr1 + 4 == ptr2,
                    "Expected ptr2 to be 4 bytes after pt1, got ptr1=0x{:08x} ptr2=0x{:08x}",
                    ptr1,
                    ptr2
                );
            }
        }
    }

    #[test]
    fn null() {
        let mut arena = [0u8; 4];
        {
            let allocator = BumpAllocator::default_single_threaded(arena.as_mut_slice());
            unsafe {
                let ptr1 = allocator.alloc(Layout::from_size_align(4, 4).unwrap()) as usize;
                assert_eq!(ptr1 % 4, 0);
                let ptr2 = allocator.alloc(Layout::from_size_align(4, 4).unwrap()) as usize;
                assert_eq!(ptr2, 0);
            }
        }
    }

    #[test]
    fn use_last_byte() {
        let mut arena = [0u8; 4];
        {
            let allocator = BumpAllocator::default_single_threaded(arena.as_mut_slice());
            unsafe {
                let ptr1 = allocator.alloc(Layout::from_size_align(3, 4).unwrap()) as usize;
                assert_eq!(ptr1 % 4, 0);
                let ptr2 = allocator.alloc(Layout::from_size_align(1, 1).unwrap()) as usize;
                assert_eq!(ptr2, arena.as_ptr().offset(3) as usize);
            }
        }
    }

    #[test]
    fn minifuzz() {
        const SIZE: usize = 1024 * 1024;

        let mut rng = thread_rng();

        for _attempts in 1..100 {
            let mut arena = Vec::with_capacity(SIZE);
            arena.resize(SIZE, 0);
            let allocator = BumpAllocator::default_single_threaded(arena.as_mut_slice());
            let mut last_ptr: Option<usize> = None;
            for _allocation in 1..10 {
                let size = rng.gen_range(1..=32);
                let alignment = 1 << rng.gen_range(1..=5);
                let layout = Layout::from_size_align(size, alignment).unwrap();
                let ptr = unsafe { allocator.alloc(layout) as usize };
                if let Some(last_ptr) = last_ptr {
                    assert!(ptr > last_ptr, "Pointer didn’t bump")
                }
                drop(last_ptr.insert(ptr));
                assert_eq!(ptr % alignment, 0);
            }
        }
    }
}
