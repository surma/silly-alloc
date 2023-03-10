#[derive(Clone, Debug)]
pub enum BumpAllocatorMemoryError {
    GrowthFailed,
    Unknown,
}

pub type BumpAllocatorMemoryResult<T> = core::result::Result<T, BumpAllocatorMemoryError>;
