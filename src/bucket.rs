use core::{
    alloc::GlobalAlloc,
    cell::UnsafeCell,
    mem::{size_of, MaybeUninit},
};

struct SegmentHeader([u32; NUM_U32_PER_HEADER]);
impl SegmentHeader {
    fn idx_to_coords(slot_idx: usize) -> (usize, usize) {
        let arr_idx = slot_idx / 32;
        let bit_idx = slot_idx % 32;
        (arr_idx, bit_idx)
    }

    fn coords_to_idx(arr_idx: usize, bit_idx: usize) -> usize {
        arr_idx * 32 | (bit_idx & (32 - 1))
    }

    fn is_free(&self, slot_idx: usize) -> bool {
        let (arr_idx, bit_idx) = Self::idx_to_coords(slot_idx);
        self.0[arr_idx] & (1 << bit_idx) > 0
    }

    fn set_slot(&mut self, slot_idx: usize) {
        let (arr_idx, bit_idx) = Self::idx_to_coords(slot_idx);
        self.0[arr_idx] |= 1 << bit_idx;
    }

    fn unset_slot(&mut self, slot_idx: usize) {
        let (arr_idx, bit_idx) = Self::idx_to_coords(slot_idx);
        self.0[arr_idx] &= !(1 << bit_idx);
    }

    fn first_free_slot(&self) -> Option<usize> {
        for (arr_idx, slots) in self.0.iter().enumerate() {
            let bit_idx = slots.leading_ones();
            if bit_idx != 32 {
                return Some(Self::coords_to_idx(arr_idx, bit_idx.try_into().unwrap()));
            }
        }
        None
    }
}

const NUM_U32_PER_HEADER: usize = 4;
pub const NUM_SLOTS_PER_SEGMENT: usize = NUM_U32_PER_HEADER * size_of::<u32>() * 8;
pub const SEGMENT_HEADER_SIZE: usize = NUM_U32_PER_HEADER * size_of::<u32>();

pub trait Slot {
    fn get(&self) -> *const u8;
    fn size() -> usize;
}

pub struct Segment<S: Slot> {
    header: [u32; NUM_U32_PER_HEADER],
    slots: [S; NUM_SLOTS_PER_SEGMENT],
}

impl<S: Slot> Segment<S> {
    pub const fn new() -> Self {
        Segment {
            header: [0u32; NUM_U32_PER_HEADER],
            slots: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }

    fn first_free_slot_idx(&self) -> Option<usize> {
        for header in self.header {
            let clo = header.leading_ones();
            if clo == 32 {
                continue;
            }
            return Some(usize::try_from(clo).unwrap());
        }
        None
    }

    fn slot_to_idx(slot_idx: usize) -> (usize, usize) {
        (slot_idx >> 5, slot_idx % 32)
    }

    fn idx_to_slot(arr_idx: usize, bit_idx: usize) -> usize {
        arr_idx << 5 | (bit_idx & 0b11111)
    }

    fn set_slot(&mut self, slot_idx: usize) {
        let (arr_idx, bit_idx) = Self::slot_to_idx(slot_idx);
        self.header[arr_idx] |= 1 << (31 - bit_idx);
    }

    fn unset_slot(&mut self, slot_idx: usize) {
        let (arr_idx, bit_idx) = Self::slot_to_idx(slot_idx);
        self.header[arr_idx] &= !(1 << (31 - bit_idx));
    }

    fn get_slot(&self, slot_idx: usize) -> *const u8 {
        self.slots[slot_idx].get()
    }

    fn default() -> Self {
        Segment {
            header: [0u32; NUM_U32_PER_HEADER],
            slots: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }
}

impl<S: Slot> Default for Segment<S> {
    fn default() -> Self {
        Segment::<S>::default()
    }
}

macro_rules! align_type {
    ($name:ident, $n:expr) => {
        #[repr(C, align($n))]
        pub struct $name<const N: usize>([u8; N]);
        impl<const N: usize> Slot for $name<N> {
            fn get(&self) -> *const u8 {
                self.0.as_ptr()
            }

            fn size() -> usize {
                N
            }
        }
    };
}

align_type!(SlotWithAlign1, 1);
align_type!(SlotWithAlign2, 2);
align_type!(SlotWithAlign4, 4);
align_type!(SlotWithAlign8, 8);
align_type!(SlotWithAlign16, 16);
align_type!(SlotWithAlign32, 32);

pub struct Bucket<S: Slot, const N: usize> {
    segments: [Segment<S>; N],
}

impl<S: Slot, const N: usize> Bucket<S, N> {
    const fn default() -> Self {
        Bucket {
            segments: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }

    fn take_first_available_slot(&mut self) -> Option<*const u8> {
        for seg in self.segments.iter_mut() {
            let Some(slot_idx) = seg.first_free_slot_idx() else {continue};
            seg.set_slot(slot_idx);
            return Some(seg.get_slot(slot_idx));
        }
        None
    }

    fn global_to_local(&self, slot_idx: usize) -> (usize, usize) {
        let seg_idx = slot_idx / NUM_SLOTS_PER_SEGMENT;
        let slot_idx = slot_idx % NUM_SLOTS_PER_SEGMENT;
        (seg_idx, slot_idx)
    }

    fn get_slot(&self, slot_idx: usize) -> *const u8 {
        let (seg_idx, slot_idx) = self.global_to_local(slot_idx);
        self.segments[seg_idx].get_slot(slot_idx)
    }

    fn set_slot(&mut self, slot_idx: usize) {
        let (seg_idx, slot_idx) = self.global_to_local(slot_idx);
        self.segments[seg_idx].set_slot(slot_idx);
    }

    fn unset_slot(&mut self, slot_idx: usize) {
        let (seg_idx, slot_idx) = self.global_to_local(slot_idx);
        self.segments[seg_idx].unset_slot(slot_idx);
    }

    fn slot_idx_for_ptr(&self, ptr: *const u8) -> Option<usize> {
        // FIXME: Do math instead
        for (seg_idx, seg) in self.segments.iter().enumerate() {
            for slot_idx in 0..NUM_SLOTS_PER_SEGMENT {
                if seg.get_slot(slot_idx) == ptr {
                    return Some(seg_idx * NUM_SLOTS_PER_SEGMENT + slot_idx);
                }
            }
        }
        None
    }
}

impl<S: Slot, const N: usize> Default for Bucket<S, N> {
    fn default() -> Self {
        Bucket::<S, N>::default()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use core::alloc::Layout;

    use anyhow::Result;

    #[test]
    fn manual_test() -> Result<()> {
        #[derive(Default)]
        struct BucketAllocator(
            UnsafeCell<Bucket<SlotWithAlign2<2>, 1>>,
            UnsafeCell<Bucket<SlotWithAlign4<4>, 1>>,
            UnsafeCell<Bucket<SlotWithAlign8<8>, 1>>,
        );

        unsafe impl GlobalAlloc for BucketAllocator {
            unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
                // FIXME: Respect align
                let size = layout.size();
                if size <= 2 {
                    if let Some(ptr) = self.0.get().as_mut().unwrap().take_first_available_slot() {
                        return ptr as *mut u8;
                    }
                }
                if size <= 4 {
                    if let Some(ptr) = self.1.get().as_mut().unwrap().take_first_available_slot() {
                        return ptr as *mut u8;
                    }
                }
                if size <= 8 {
                    if let Some(ptr) = self.2.get().as_mut().unwrap().take_first_available_slot() {
                        return ptr as *mut u8;
                    }
                }
                core::ptr::null_mut()
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                // FIXME: Respect align
                if let Some(bucket) = self.0.get().as_mut() {
                    if let Some(slot_idx) = bucket.slot_idx_for_ptr(ptr) {
                        bucket.unset_slot(slot_idx);
                    }
                }
            }
        }

        let b = BucketAllocator::default();
        unsafe {
            let ptr1 = b.alloc(Layout::from_size_align(2, 1)?);
            let ptr2 = b.alloc(Layout::from_size_align(2, 1)?);
            assert!(!ptr1.is_null());
            assert!(!ptr2.is_null());
            assert_eq!(ptr1.offset(2), ptr2);
        }
        Ok(())
    }
}
