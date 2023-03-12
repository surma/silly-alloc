extern crate alloc;
use alloc::alloc::alloc;
use alloc::boxed::Box;
use bytemuck::Zeroable;
use core::alloc::Layout;

pub trait OnHeap {
    fn on_heap() -> Box<Self>;
}

impl<T: Zeroable> OnHeap for T {
    fn on_heap() -> Box<Self> {
        unsafe {
            let layout = Layout::new::<T>();
            let ptr = alloc(layout);
            Box::from_raw(ptr as *mut T)
        }
    }
}
