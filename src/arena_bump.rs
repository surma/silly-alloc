use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    fmt::Debug,
    ptr::null_mut,
};

use crate::head::{Head, SingleThreadedHead};

/// A bump allocator that works on a specified arena of size N.
pub struct ArenaBumpAllocator<const N: usize, H: Head = SingleThreadedHead> {
    arena: [u8; N],
    head: UnsafeCell<Option<H>>,
}

impl<const N: usize, H: Head + Default> Debug for ArenaBumpAllocator<N, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let head: i64 = unsafe { &*self.head.get() }
            .as_ref()
            .map(|h| h.current().try_into().unwrap())
            .unwrap_or(-1);
        f.debug_struct("ArenaBumpAllocator")
            .field("size", &N)
            .field("head", &head)
            .finish()
    }
}

unsafe impl<const N: usize, H: Head + Default> Sync for ArenaBumpAllocator<N, H> {}

impl<const N: usize, H: Head + Default> ArenaBumpAllocator<N, H> {
    pub const fn new() -> ArenaBumpAllocator<N, H> {
        ArenaBumpAllocator {
            arena: [0u8; N],
            head: UnsafeCell::new(None),
        }
    }

    pub unsafe fn reset(&self) {
        if let Some(head) = self.try_as_head_mut() {
            head.set(0);
        }
    }

    fn get_head_ptr(&self) -> Option<*const u8> {
        self.arena
            .get(self.as_head_mut().current())
            .map(|p| p as *const u8)
    }

    fn try_as_head_mut(&self) -> Option<&mut H> {
        unsafe { &mut *self.head.get() }.as_mut()
    }

    fn as_head_mut(&self) -> &mut H {
        self.try_as_head_mut().unwrap()
    }

    unsafe fn ensure_init(&self) {
        let head = &mut *self.head.get();
        if head.is_some() {
            return;
        }
        drop(head.insert(H::default()));
    }
}

unsafe impl<const N: usize, H: Head + Default> GlobalAlloc for ArenaBumpAllocator<N, H> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.ensure_init();

        let align = layout.align();
        let size = layout.size();
        let ptr = match self.get_head_ptr() {
            Some(ptr) => ptr,
            _ => return null_mut(),
        };
        let offset = ptr.align_offset(align);
        self.as_head_mut().add(offset + size);
        ptr.offset(offset.try_into().unwrap()) as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{thread_rng, Rng};

    const SIZE: usize = 64 * 1024 * 1024;

    #[test]
    fn minifuzz() {
        unsafe {
            let mut rng = thread_rng();
            static mut ALLOCATOR: ArenaBumpAllocator<SIZE> = ArenaBumpAllocator::<SIZE>::new();
            for _attempts in 1..100 {
                ALLOCATOR.reset();
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
