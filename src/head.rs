use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub trait Head {
    fn current(&self) -> usize;
    fn add(&self, inc: usize);
    fn set(&self, v: usize);
}

use bytemuck::Zeroable;

pub struct ThreadSafeHead(AtomicUsize);

unsafe impl Zeroable for ThreadSafeHead {}

impl ThreadSafeHead {
    pub const fn new() -> Self {
        ThreadSafeHead(AtomicUsize::new(0))
    }
}

impl Head for ThreadSafeHead {
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

impl Default for ThreadSafeHead {
    fn default() -> Self {
        ThreadSafeHead(AtomicUsize::new(0))
    }
}

pub struct SingleThreadedHead(UnsafeCell<usize>);

unsafe impl Zeroable for SingleThreadedHead {}
unsafe impl Sync for SingleThreadedHead {}

impl SingleThreadedHead {
    pub const fn new() -> Self {
        SingleThreadedHead(UnsafeCell::new(0))
    }
}

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
