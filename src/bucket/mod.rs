/*!
Bucket allocators.

A bucket allocator defines multiple buckets where each item has the same size. The number and granularity of the buckets can be tuned to what is typical allocation behavior of the app at hand. In contrast to bump allocators, bucket allocators can also free memory.

# Examples

You can create a custom bucket allocator by writing a (pseudo) struct that contains the parameters for all buckets:

```rust
use silly_alloc::bucket_allocator;

#[bucket_allocator]
struct MyBucketAllocator {
    vec2: Bucket<SlotSize<2>, NumSlots<128>, Align<2>>,
    overflow: Bucket<SlotSize<64>, NumSlots<64>, Align<64>>
}
```

Note that these types and generics are not really in use. They are merely there for an idiomatic and syntactically plausible way to provide the parameters. The macro rewrites this struct definition to another one using different types.

The new bucket allocator can then be instantiated and used as a global allocator as per usual:

```rust
# use silly_alloc::bucket_allocator;
#
# #[bucket_allocator]
# struct MyBucketAllocator {
#     vec2: Bucket<SlotSize<2>, NumSlots<128>, Align<2>>,
#     overflow: Bucket<SlotSize<64>, NumSlots<64>, Align<64>>
# }
#[global_allocator]
static ALLOCATOR: MyBucketAllocator = MyBucketAllocator::new();
```

Buckets are checked for the best fit in order of specification. Full buckets are skipped.

# Technical details

A bucket is defined by three parameters:

- The size of an item in the bucket
- The number of items that fit in the bucket
- An optional alignment constraint

The speed of bucket allocators stems from the fact that all items in the bucket are the same size, and as such a simple bit mask is enough to track if a “slot” is in use or not. For simplicity, 32 slots a grouped into one segment, where a single `u32` is used to track which slot has already been allocated. A bucket, as a consequence, is a series of segments. This also implies that the size of the bucket will be rounded up to the next multiple of 32.
*/
use core::{
    fmt::{Debug, Formatter},
    marker::PhantomData,
    mem::{size_of, MaybeUninit},
};

use bytemuck::Zeroable;

// TODO: Implement thread-safe segments
// #[cfg(target_feature = "feature")]
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
align_type!(SlotWithAlign64, 64);
align_type!(SlotWithAlign128, 128);
align_type!(SlotWithAlign256, 256);
align_type!(SlotWithAlign512, 512);

#[derive(Debug, Clone, Copy)]
pub struct BucketImpl<S: Slot, const N: usize> {
    // Deeply saddened that I had to introduce a bool flag here. There just currently is no way initializing this array in a const way, so I have to defer it to runtime and track when it has been done. This flag can be removed when at least one of these options is available in stable rust:
    // - const fn can be in traits
    // - core::ptr::write_bytes is const
    // - core::mem::MaybeUninit.zeroed() is const (although that probably relies on previous point)
    is_init: bool,
    segments: MaybeUninit<[Segment<S>; N]>,
}

impl<S: Slot, const NUM_SEGMENTS: usize> BucketImpl<S, NUM_SEGMENTS> {
    pub const fn new() -> Self {
        Self {
            is_init: false,
            segments: MaybeUninit::uninit(),
        }
        // unsafe { MaybeUninit::zeroed().assume_init() }
    }

    pub fn ensure_init(&mut self) {
        if self.is_init {
            return;
        }
        unsafe {
            core::ptr::write_bytes(self.segments.as_mut_ptr(), 0u8, 1);
        }
        self.is_init = true
    }

    fn get_segments(&self) -> &[Segment<S>; NUM_SEGMENTS] {
        assert!(self.is_init);
        unsafe { self.segments.assume_init_ref() }
    }

    fn get_segments_mut(&mut self) -> &mut [Segment<S>; NUM_SEGMENTS] {
        assert!(self.is_init);
        unsafe { self.segments.assume_init_mut() }
    }

    pub fn claim_first_available_slot(&mut self) -> Option<*const u8> {
        for seg in self.get_segments_mut().iter_mut() {
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
        self.get_segments()[seg_idx].get_slot(slot_idx)
    }

    pub fn set_slot(&mut self, slot_idx: usize) {
        let (seg_idx, slot_idx) = self.global_to_local(slot_idx);
        self.get_segments_mut()[seg_idx].header.set_slot(slot_idx);
    }

    pub fn unset_slot(&mut self, slot_idx: usize) {
        let (seg_idx, slot_idx) = self.global_to_local(slot_idx);
        self.get_segments_mut()[seg_idx].header.unset_slot(slot_idx);
    }

    pub fn slot_idx_for_ptr(&self, ptr: *const u8) -> Option<usize> {
        let seg_stride = size_of::<Segment<S>>();
        let slot_stride = size_of::<S>();

        let start = self.segments.as_ptr() as *const u8;
        let offset = unsafe { ptr.offset_from(start) };
        // Conversion to usize will only succeed for positive numbers.
        // If it's negative, ptr is in previous segment.
        let offset = usize::try_from(offset).ok()?;
        let seg_idx = offset / seg_stride;
        let offset = offset % seg_stride;
        let slot_idx = offset / slot_stride;
        if seg_idx >= NUM_SEGMENTS || slot_idx > NUM_SLOTS_PER_SEGMENT {
            return None;
        }

        Some(seg_idx * NUM_SLOTS_PER_SEGMENT + slot_idx)
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

    use silly_alloc_macros::bucket_allocator;

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

    #[test]
    fn alignment() -> Result<()> {
        let mut b = MyBucketAllocator::new();
        unsafe {
            let layout = Layout::from_size_align(2, 8)?;
            let ptr1 = b.alloc(layout);
            // Alignment requirement should force the allocation into the last bucket desipte its size
            assert!(ptr1 >= &mut b.2 as *mut _ as *mut u8);
        }
        Ok(())
    }

    #[test]
    fn alignment_fail() -> Result<()> {
        let b = MyBucketAllocator::new();
        unsafe {
            let layout = Layout::from_size_align(2, 32)?;
            let ptr1 = b.alloc(layout);
            assert!(ptr1.is_null());
        }
        Ok(())
    }

    #[test]
    fn first_alloc_in_late_bucket() -> Result<()> {
        unsafe {
            // Rust doesn't guarantee field order in structs, so I don't know whether the 2byte bucket or the 8byte bucket comes first. So I am gonna test both, as they both have to work anyway.
            let layouts = vec![
                Layout::from_size_align(2, 1)?,
                Layout::from_size_align(8, 1)?,
            ];
            for layout in layouts {
                let b = MyBucketAllocator::new();
                let _ptr1 = b.alloc(layout.clone());
                let ptr2 = b.alloc(layout.clone());
                let _ptr3 = b.alloc(layout.clone());
                b.dealloc(ptr2, layout);
                let ptr4 = b.alloc(layout.clone());
                assert_eq!(ptr2, ptr4);
            }
        }
        Ok(())
    }

    #[test]
    #[ignore]
    fn unsorted_buckets() -> Result<()> {
        #[bucket_allocator]
        struct MyBucketAllocator {
            vec8: Bucket<SlotSize<8>, NumSlots<32>, Align<8>>,
            vec2: Bucket<SlotSize<2>, NumSlots<32>, Align<8>>,
        }

        unsafe {
            let b = MyBucketAllocator::new();
            let ptr1 = b.alloc(Layout::from_size_align(2, 2)?);
            let ptr2 = b.alloc(Layout::from_size_align(8, 8)?);
            // Assert the two allocations are in different buckets
            assert!(ptr1.offset_from(ptr2).abs() > 8);
        }
        Ok(())
    }
}
