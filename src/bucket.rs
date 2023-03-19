use core::{
    fmt::{Debug, Formatter},
    marker::PhantomData,
    mem::{size_of, MaybeUninit},
};

// TODO: Implement thread-safe segments
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

const NUM_U32_PER_HEADER: usize = 1;
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

impl<S: Slot> Debug for Segment<S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("Segment {")?;
        for header in self.header {
            f.write_fmt(format_args!("{:08b}", (header >> 24) & 0xff))?;
            f.write_str("_")?;
            f.write_fmt(format_args!("{:08b}", (header >> 16) & 0xff))?;
            f.write_str("_")?;
            f.write_fmt(format_args!("{:08b}", (header >> 8) & 0xff))?;
            f.write_str("_")?;
            f.write_fmt(format_args!("{:08b}", (header >> 0) & 0xff))?;
        }
        f.write_str("}")?;
        Ok(())
    }
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
        Segment::<S>::new()
    }
}

macro_rules! align_type {
    ($name:ident, $n:expr) => {
        #[derive(Debug)]
        #[repr(C, align($n))]
        pub struct $name<const N: usize>([u8; N]);
        impl<const N: usize> $name<N> {
            pub const fn new() -> Self {
                $name([0u8; N])
            }
        }

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

#[derive(Debug)]
pub struct BucketImpl<S: Slot, const N: usize> {
    segments: [Segment<S>; N],
}

impl<S: Slot, const N: usize> BucketImpl<S, N> {
    pub const fn new() -> Self {
        BucketImpl {
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

pub struct SlotSize<const N: usize>;
pub struct NumSlots<const N: usize>;
pub struct Align<const N: usize>;

pub struct Bucket<S, N, A = Align<1>>(PhantomData<S>, PhantomData<N>, PhantomData<A>);

impl<S: Slot, const N: usize> Default for BucketImpl<S, N> {
    fn default() -> Self {
        BucketImpl::<S, N>::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use core::alloc::{GlobalAlloc, Layout};

    use anyhow::Result;

    use wasm_alloc_macros::bucket_allocator;

    #[bucket_allocator]
    struct MyBucketAllocator {
        vec2: Bucket<SlotSize<2>, NumSlots<32>, Align<2>>,
        vec4: Bucket<SlotSize<4>, NumSlots<32>, Align<4>>,
        vec8: Bucket<SlotSize<8>, NumSlots<32>, Align<8>>,
    }

    #[test]
    fn next_in_bucket() -> Result<()> {
        let b = MyBucketAllocator::new();
        unsafe {
            let ptr1 = b.alloc(Layout::from_size_align(2, 1)?);
            let ptr2 = b.alloc(Layout::from_size_align(2, 1)?);
            assert!(!ptr1.is_null());
            assert!(!ptr2.is_null());
            assert_eq!(ptr1.offset(2), ptr2);
        }
        Ok(())
    }

    #[test]
    fn reuse() -> Result<()> {
        let b = MyBucketAllocator::new();
        unsafe {
            let layout = Layout::from_size_align(2, 1)?;
            let ptr1 = b.alloc(layout.clone());
            let ptr2 = b.alloc(layout.clone());
            let ptr3 = b.alloc(layout.clone());
            assert_eq!(ptr1.offset(2), ptr2);
            assert_eq!(ptr2.offset(2), ptr3);
            b.dealloc(ptr2, layout);
            let ptr4 = b.alloc(layout.clone());
            assert_eq!(ptr2, ptr4);
        }
        Ok(())
    }

    #[test]
    fn bucket_overflow() -> Result<()> {
        let b = MyBucketAllocator::new();
        unsafe {
            let layout = Layout::from_size_align(2, 1)?;
            // Fill 2 byte bucket
            for _ in 0..32 {
                b.alloc(layout.clone());
            }
            let ptr1 = b.alloc(Layout::from_size_align(4, 1)?);
            let ptr2 = b.alloc(layout.clone());
            assert_eq!(ptr1.offset(4), ptr2);
        }
        Ok(())
    }
}
