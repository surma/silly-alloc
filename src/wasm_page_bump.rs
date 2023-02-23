// LLVM will set the address of `__heap_base` to the start of the heap area.
extern "C" {
    static __heap_base: u8;
}

use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    marker::PhantomData,
    ptr::null_mut,
};

use crate::head::{Head, SingleThreadedHead};

pub trait MemoryOperations {
    fn current_size() -> usize;
    fn grow(bytes: usize);
}

pub struct WasmPageOperations;

const PAGE_SIZE: usize = 64 * 1024;
impl MemoryOperations for WasmPageOperations {
    fn current_size() -> usize {
        core::arch::wasm32::memory_size(0) * PAGE_SIZE
    }

    fn grow(bytes: usize) {
        let delta_pages_f = (bytes as f32) / (PAGE_SIZE as f32);
        let mut delta_pages: usize = delta_pages_f as usize;
        if delta_pages_f % 1.0 != 0.0 {
            delta_pages += 1;
        }
        let new_num_pages = core::arch::wasm32::memory_grow(0, delta_pages);
        if new_num_pages == usize::MAX {
            core::arch::wasm32::unreachable();
        }
    }
}

pub struct WasmPageBumpAllocator<
    H: Head = SingleThreadedHead,
    M: MemoryOperations = WasmPageOperations,
> {
    size: UnsafeCell<usize>,
    head: UnsafeCell<Option<H>>,
    _memory_ops: PhantomData<M>,
}

unsafe impl<H: Head + Default, M: MemoryOperations> Sync for WasmPageBumpAllocator<H, M> {}

impl<H: Head + Default, M: MemoryOperations> WasmPageBumpAllocator<H, M> {
    pub const fn new() -> Self {
        WasmPageBumpAllocator {
            size: UnsafeCell::new(0),
            head: UnsafeCell::new(None),
            _memory_ops: PhantomData,
        }
    }

    fn try_as_head_mut(&self) -> Option<&mut H> {
        unsafe { &mut *self.head.get() }.as_mut()
    }

    pub fn reset(&self) {
        if let Some(head) = self.try_as_head_mut() {
            head.set(unsafe { &__heap_base as *const u8 as usize });
        }
    }

    unsafe fn ensure_init(&self) {
        let head = &mut *self.head.get();
        if head.is_some() {
            return;
        }

        *self.size.get() = M::current_size();
        drop(head.insert(H::default()));
        self.reset();
    }
}

unsafe impl<H: Head + Default, M: MemoryOperations> GlobalAlloc for WasmPageBumpAllocator<H, M> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let c = || -> Option<*mut u8> {
            self.ensure_init();

            let align = layout.align();
            let size = layout.size();
            // `ensure_init` make sure we have a head.
            let head = self.try_as_head_mut()?;
            let ptr = head.current() as *const u8;
            let offset = ptr.align_offset(align);
            if ptr.offset((offset + size).try_into().ok()?) as usize >= M::current_size() {
                M::grow(offset + size);
            }
            head.add(offset + size);
            Some(ptr.offset(offset.try_into().ok()?) as *mut u8)
        };
        c().unwrap_or(null_mut())
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
