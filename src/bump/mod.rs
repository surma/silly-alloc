/*!

Bump allocators.

Bump allocators work on a linear chunk of memory and only store a pointer where the next available byte is. New allocations are made by moving that pointer forwards, which is easy and fast. The downside is that memory cannot be freed and reused, so it should be used for short-lived programs.

# Examples

## Using an array or slice as heap

```rust
use silly_alloc::SliceBumpAllocator;

const ARENA_SIZE: usize = 64 * 1024 * 1024;
static arena: [u8; ARENA_SIZE] = [0u8; ARENA_SIZE];

#[global_allocator]
static ALLOCATOR: SliceBumpAllocator = SliceBumpAllocator::with_slice(arena.as_slice());
```

## Using the entire WebAssembly Memory as heap

```rust
use silly_alloc::WasmBumpAllocator;

#[global_allocator]
static ALLOCATOR: WasmBumpAllocator = WasmBumpAllocator::with_memory();
```

Note that `WasmBumpAllocator` respects the heap start address that is provided by the linker, making sure `static`s and other data doesn’t get corrupted by runtime allocations.

*/
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    fmt::Debug,
    marker::PhantomData,
    ptr::null_mut,
};

pub mod head;
pub use head::{Head, SingleThreadedHead, ThreadSafeHead};

#[cfg(target_arch = "wasm32")]
pub mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::{ThreadsafeWasmBumpAllocator, WasmBumpAllocator};

/// Trait to model a consecutive chunk of linear memory for bump allocators.
pub trait BumpAllocatorArena {
    /// Returns the first pointer in the arena.
    fn start(&self) -> *const u8;
    /// Returns the current size of the arena in bytes.
    fn size(&self) -> usize;
    /// Ensures that the arena is at least `min_size` bytes big, or returns an error if that is not possible.
    fn ensure_min_size(&self, min_size: usize) -> BumpAllocatorArenaResult<usize>;
    /// Returns the number of bytes `ptr` is pointing past the end of the arena. Returns `None` if `ptr` is not pointing past the end.
    fn past_end(&self, ptr: *const u8) -> Option<usize> {
        // FIXME: This is probably not the best way to handle big memory sizes that overflow isize. Currently this panics whenever we can’t convert an `isize` to an `usize`.
        let v = unsafe {
            self.start()
                .offset(self.size().try_into().unwrap())
                .offset_from(ptr)
        };
        if v > 0 {
            None
        } else {
            Some(v.abs().try_into().unwrap())
        }
    }
}

#[derive(Clone, Debug)]
pub enum BumpAllocatorArenaError {
    GrowthFailed,
    Unknown,
}

pub type BumpAllocatorArenaResult<T> = core::result::Result<T, BumpAllocatorArenaError>;

impl BumpAllocatorArena for &[u8] {
    fn start(&self) -> *const u8 {
        self.as_ptr()
    }

    fn size(&self) -> usize {
        self.len()
    }

    fn ensure_min_size(&self, _min_size: usize) -> BumpAllocatorArenaResult<usize> {
        Err(BumpAllocatorArenaError::GrowthFailed)
    }
}

/// A generic bump allocator.
///
/// The bump allocator works on memory `M`, tracking where the remaining
/// free memory starts using head `H`.
pub struct BumpAllocator<'a, M: BumpAllocatorArena = &'a [u8], H: Head = SingleThreadedHead> {
    head: UnsafeCell<H>,
    memory: M,
    lifetime: PhantomData<&'a u8>,
}

/// A `BumpAllocator` that uses the given byte slice as the arena.
pub type SliceBumpAllocator<'a> = BumpAllocator<'a, &'a [u8], SingleThreadedHead>;

impl<'a> SliceBumpAllocator<'a> {
    pub const fn with_slice(arena: &'a [u8]) -> SliceBumpAllocator<'a> {
        BumpAllocator {
            memory: arena,
            head: UnsafeCell::new(SingleThreadedHead::new()),
            lifetime: PhantomData,
        }
    }
}

/// A `BumpAllocator` that uses the given slice as the arena.
pub type ThreadsafeSliceBumpAllocator<'a> = BumpAllocator<'a, &'a [u8], ThreadSafeHead>;

impl<'a> ThreadsafeSliceBumpAllocator<'a> {
    pub const fn with_slice(arena: &'a [u8]) -> ThreadsafeSliceBumpAllocator<'a> {
        BumpAllocator {
            memory: arena,
            head: UnsafeCell::new(ThreadSafeHead::new()),
            lifetime: PhantomData,
        }
    }
}

impl<'a, M: BumpAllocatorArena, H: Head + Default> BumpAllocator<'a, M, H> {
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

    pub fn arena(&self) -> &dyn BumpAllocatorArena {
        &self.memory
    }
}

impl<'a, M: BumpAllocatorArena, H: Head + Default> Debug for BumpAllocator<'a, M, H> {
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

unsafe impl<'a, M: BumpAllocatorArena, H: Head> Sync for BumpAllocator<'a, M, H> {}

unsafe impl<'a, M: BumpAllocatorArena, H: Head + Default> GlobalAlloc for BumpAllocator<'a, M, H> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();
        let ptr = match self.get_head_ptr() {
            Some(ptr) => ptr,
            _ => return null_mut(),
        };
        let offset = ptr.align_offset(align);
        let head = self.as_head_mut();
        let last_byte_of_new_allocation = self.memory.start().offset(
            (head.num_bytes_used() + offset + size - 1)
                .try_into()
                .unwrap(),
        );
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
    use std::vec::Vec;
    use xorshift;

    #[test]
    fn increment() {
        let arena = [0u8; 1024];
        {
            let allocator = SliceBumpAllocator::with_slice(arena.as_slice());
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
        let arena = [0u8; 4];
        {
            let allocator = SliceBumpAllocator::with_slice(arena.as_slice());
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
        let arena = [0u8; 4];
        {
            let allocator = SliceBumpAllocator::with_slice(arena.as_slice());
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

        use xorshift::{Rng, SeedableRng};
        let mut rng = xorshift::Xoroshiro128::from_seed(&[1u64, 2, 3, 4]);

        for _attempts in 1..100 {
            let mut arena = Vec::with_capacity(SIZE);
            arena.resize(SIZE, 0);
            let allocator = SliceBumpAllocator::with_slice(arena.as_slice());
            let mut last_ptr: Option<usize> = None;
            for _allocation in 1..10 {
                let size = rng.gen_range(1, 32);
                let alignment = 1 << rng.gen_range(1, 5);
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
