// #![no_std]

use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    ptr::null_mut,
};

mod head;

use head::{Head, SingleThreadedHead};

/// A bump allocator that works on a specified arena of size N.
///
/// RT]}
pub struct ArenaBumpAllocator<const N: usize, H: Head = SingleThreadedHead> {
    arena: [u8; N],
    head: UnsafeCell<Option<H>>,
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
        self.get_head_mut().set(0);
    }

    unsafe fn get_head_ptr(&self) -> Option<*const u8> {
        match self.arena.get(self.get_head_mut().current()) {
            Some(ptr) => Some(ptr as *const u8),
            _ => None,
        }
    }

    unsafe fn get_head_mut(&self) -> &mut H {
        println!(">>>>>>> {:?}", (*self.head.get()).is_some());
        (*self.head.get()).as_mut().unwrap()
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
        self.get_head_mut().add(offset + size);
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
        let mut rng = thread_rng();
        unsafe {
            for _attempts in 1..100 {
                let ALLOCATOR: ArenaBumpAllocator<SIZE> = ArenaBumpAllocator::<SIZE>::new();
                for _allocation in 1..10 {
                    let size = rng.gen_range(1..=32);
                    let alignment = rng.gen_range(1..=32);
                    let layout = Layout::from_size_align(size, alignment).unwrap();
                    let ptr = ALLOCATOR.alloc(layout) as usize;
                    assert!(ptr % alignment == 0);
                }
            }
        }
    }
}
