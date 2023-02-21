use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub trait Head {
    fn current(&self) -> usize;
    fn add(&self, inc: usize);
    fn set(&self, v: usize);
}

pub struct ThreadedSafeHead(AtomicUsize);

impl Head for ThreadedSafeHead {
    fn current(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }

    fn add(&self, inc: usize) {
        self.0.fetch_add(inc, Ordering::SeqCst);
    }

    fn set(&self, v: usize) {
        self.0.store(v, Ordering::SeqCst);
    }
}

impl Default for ThreadedSafeHead {
    fn default() -> Self {
        ThreadedSafeHead(AtomicUsize::new(0))
    }
}

pub struct SingleThreadedHead(UnsafeCell<usize>);

unsafe impl Sync for SingleThreadedHead {}

impl Head for SingleThreadedHead {
    fn current(&self) -> usize {
        unsafe { *self.0.get() }
    }

    fn add(&self, inc: usize) {
        unsafe {
            *self.0.get() = self.current() + inc;
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
