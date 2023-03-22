use core::{
    fmt::{Debug, Formatter},
    marker::PhantomData,
    mem::size_of,
};

use bytemuck::Zeroable;

// TODO: Implement thread-safe segments
#[derive(Clone, Copy)]
struct SegmentHeader([u32; NUM_U32_PER_HEADER]);
impl SegmentHeader {
    const fn new() -> Self {
        SegmentHeader([0u32; NUM_U32_PER_HEADER])
    }

    fn first_free_slot_idx(&self) -> Option<usize> {
        for header in self.0 {
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

    fn set_slot(&mut self, slot_idx: usize) {
        let (arr_idx, bit_idx) = Self::slot_to_idx(slot_idx);
        self.0[arr_idx] |= 1 << (31 - bit_idx);
    }

    fn unset_slot(&mut self, slot_idx: usize) {
        let (arr_idx, bit_idx) = Self::slot_to_idx(slot_idx);
        self.0[arr_idx] &= !(1 << (31 - bit_idx));
    }
}

unsafe impl Zeroable for SegmentHeader {}

impl Debug for SegmentHeader {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("SegmentHeader {")?;
        for header in self.0 {
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

const NUM_U32_PER_HEADER: usize = 1;
pub const NUM_SLOTS_PER_SEGMENT: usize = NUM_U32_PER_HEADER * size_of::<u32>() * 8;
pub const SEGMENT_HEADER_SIZE: usize = NUM_U32_PER_HEADER * size_of::<u32>();

pub trait Slot: Copy + Default {
    fn get(&self) -> *const u8;
    fn size() -> usize;
}

#[derive(Clone, Copy)]
pub struct Segment<S: Slot> {
    header: SegmentHeader,
    slots: [S; NUM_SLOTS_PER_SEGMENT],
}

impl<S: Slot> Segment<S> {
    fn new() -> Self {
        Segment {
            header: SegmentHeader::new(),
            slots: [S::default(); NUM_SLOTS_PER_SEGMENT],
        }
    }

    fn get_slot(&self, slot_idx: usize) -> *const u8 {
        self.slots[slot_idx].get()
    }
}

impl<S: Slot> Default for Segment<S> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<S: Slot + Zeroable> Zeroable for Segment<S> {}

impl<S: Slot> Debug for Segment<S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Segment")
            .field("header", &self.header)
            .finish()?;
        Ok(())
    }
}

macro_rules! align_type {
    ($name:ident, $n:expr) => {
        #[derive(Debug, Clone, Copy)]
        #[repr(C, align($n))]
        pub struct $name<const N: usize>([u8; N]);
        impl<const N: usize> $name<N> {
            pub const fn new() -> Self {
                $name([0u8; N])
            }
        }

        impl<const N: usize> Default for $name<N> {
            fn default() -> Self {
                Self::new()
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

        unsafe impl<const N: usize> Zeroable for $name<N> {}
    };
}

align_type!(SlotWithAlign1, 1);
align_type!(SlotWithAlign2, 2);
align_type!(SlotWithAlign4, 4);
align_type!(SlotWithAlign8, 8);
align_type!(SlotWithAlign16, 16);
align_type!(SlotWithAlign32, 32);

#[derive(Debug, Clone, Copy)]
pub struct BucketImpl<S: Slot, const N: usize> {
    // Deeply saddened that I had to introduce an `Option` here. There just currently is no way initializing this array in a const way. This `Option` can be removed when at least one of these options is available:
    // - const fn can be in traits
    // - core::ptr::write_bytes is const and stable
    // - core::mem::MaybeUninit.zeroed() is const and stable (although that probably relies on the above)
    segments: Option<[Segment<S>; N]>,
}

impl<S: Slot, const N: usize> BucketImpl<S, N> {
    pub const fn new() -> Self {
        Self { segments: None }
        // unsafe { MaybeUninit::zeroed().assume_init() }
    }

    pub fn ensure_init(&mut self) {
        if self.segments.is_some() {
            return;
        }
        self.segments = Some([Segment::<S>::default(); N]);
    }

    pub fn claim_first_available_slot(&mut self) -> Option<*const u8> {
        for seg in self.segments.as_mut().unwrap().iter_mut() {
            let Some(slot_idx) = seg.header.first_free_slot_idx() else {continue};
            seg.header.set_slot(slot_idx);
            return Some(seg.get_slot(slot_idx));
        }
        None
    }

    fn global_to_local(&self, slot_idx: usize) -> (usize, usize) {
        let seg_idx = slot_idx / NUM_SLOTS_PER_SEGMENT;
        let slot_idx = slot_idx % NUM_SLOTS_PER_SEGMENT;
        (seg_idx, slot_idx)
    }

    pub fn get_slot(&self, slot_idx: usize) -> *const u8 {
        let (seg_idx, slot_idx) = self.global_to_local(slot_idx);
        self.segments.as_ref().unwrap()[seg_idx].get_slot(slot_idx)
    }

    pub fn set_slot(&mut self, slot_idx: usize) {
        let (seg_idx, slot_idx) = self.global_to_local(slot_idx);
        self.segments.as_mut().unwrap()[seg_idx]
            .header
            .set_slot(slot_idx);
    }

    pub fn unset_slot(&mut self, slot_idx: usize) {
        let (seg_idx, slot_idx) = self.global_to_local(slot_idx);
        self.segments.as_mut().unwrap()[seg_idx]
            .header
            .unset_slot(slot_idx);
    }

    pub fn slot_idx_for_ptr(&self, ptr: *const u8) -> Option<usize> {
        // FIXME: Do math instead
        for (seg_idx, seg) in self.segments.as_ref().unwrap().iter().enumerate() {
            for slot_idx in 0..NUM_SLOTS_PER_SEGMENT {
                if seg.get_slot(slot_idx) == ptr {
                    return Some(seg_idx * NUM_SLOTS_PER_SEGMENT + slot_idx);
                }
            }
        }
        None
    }
}

unsafe impl<S: Slot, const N: usize> Zeroable for BucketImpl<S, N> {}

impl<S: Slot, const N: usize> Default for BucketImpl<S, N> {
    fn default() -> Self {
        BucketImpl::<S, N>::new()
    }
}

pub struct SlotSize<const N: usize>;
pub struct NumSlots<const N: usize>;
pub struct Align<const N: usize>;

pub struct Bucket<S, N, A = Align<1>>(PhantomData<S>, PhantomData<N>, PhantomData<A>);

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
