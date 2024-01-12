/*!
Heads track where the first free byte in an arena is.
*/

use core::cell::UnsafeCell;

/// The head is the pointer that gets bumped in a bump allocator.
/// It tracks of how many bytes have been marked as in-use.
pub trait Head {
    fn num_bytes_used(&self) -> usize;
    fn bump(&self, inc: usize);
    fn set(&self, v: usize);
}

#[cfg(feature = "atomics")]
mod atomics {
    use super::Head;
    use core::sync::atomic::{AtomicUsize, Ordering};
    pub struct ThreadSafeHead(AtomicUsize);

    impl ThreadSafeHead {
        pub const fn new() -> Self {
            ThreadSafeHead(AtomicUsize::new(0))
        }
    }

    impl Head for ThreadSafeHead {
        fn num_bytes_used(&self) -> usize {
            self.0.load(Ordering::SeqCst)
        }

        fn bump(&self, inc: usize) {
            self.0.fetch_add(inc, Ordering::SeqCst);
        }

        fn set(&self, v: usize) {
            self.0.store(v, Ordering::SeqCst);
        }
    }

    impl Default for ThreadSafeHead {
        fn default() -> Self {
            ThreadSafeHead(AtomicUsize::new(0))
        }
    }
}
#[cfg(feature = "atomics")]
pub use atomics::*;

pub struct SingleThreadedHead(UnsafeCell<usize>);

unsafe impl Sync for SingleThreadedHead {}

impl SingleThreadedHead {
    pub const fn new() -> Self {
        SingleThreadedHead(UnsafeCell::new(0))
    }
}

impl Head for SingleThreadedHead {
    fn num_bytes_used(&self) -> usize {
        unsafe { *self.0.get() }
    }

    fn bump(&self, inc: usize) {
        unsafe {
            *self.0.get() = self.num_bytes_used() + inc;
        }
    }

    fn set(&self, v: usize) {
        unsafe {
            *self.0.get() = v;
        }
    }
}

impl Default for SingleThreadedHead {
    fn default() -> Self {
        SingleThreadedHead(UnsafeCell::new(0))
    }
}
