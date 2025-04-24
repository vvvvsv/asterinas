// SPDX-License-Identifier: MPL-2.0

use crate::allocator::CommonSizeClass;
use alloc::vec::Vec;
use ostd::{
    cpu::local::{CpuLocal, DynCpuLocalChunk, DynCpuLocalDealloc, DynamicStorage},
    prelude::*,
    sync::SpinLock,
    Error,
};

/// Allocator for CPU-Local objects.
struct CpuLocalAllocator<const ITEM_SIZE: usize> {
    chunks: SpinLock<Vec<DynCpuLocalChunk<ITEM_SIZE>>>,
}

/// Global allocators for CPU-Local objects.
static ALLOCATOR_8: CpuLocalAllocator<8> = CpuLocalAllocator::new();
static ALLOCATOR_16: CpuLocalAllocator<16> = CpuLocalAllocator::new();
static ALLOCATOR_32: CpuLocalAllocator<32> = CpuLocalAllocator::new();
static ALLOCATOR_64: CpuLocalAllocator<64> = CpuLocalAllocator::new();
static ALLOCATOR_128: CpuLocalAllocator<128> = CpuLocalAllocator::new();
static ALLOCATOR_256: CpuLocalAllocator<256> = CpuLocalAllocator::new();
static ALLOCATOR_512: CpuLocalAllocator<512> = CpuLocalAllocator::new();
static ALLOCATOR_1024: CpuLocalAllocator<1024> = CpuLocalAllocator::new();
static ALLOCATOR_2048: CpuLocalAllocator<2048> = CpuLocalAllocator::new();

impl<const ITEM_SIZE: usize> CpuLocalAllocator<ITEM_SIZE> {
    const fn new() -> Self {
        Self {
            chunks: SpinLock::new(Vec::new()),
        }
    }

    fn alloc<T>(&'static self) -> Result<CpuLocal<T, DynamicStorage<T>>> {
        debug_assert!(core::mem::size_of::<T>() <= ITEM_SIZE);
        let mut chunks = self.chunks.lock();
        for chunk in chunks.iter_mut() {
            if let Some(cpu_local) = chunk.try_alloc::<T>(self) {
                return Ok(cpu_local);
            }
        }
        let mut new_chunk = DynCpuLocalChunk::<ITEM_SIZE>::new()?;
        let cpu_local = new_chunk.try_alloc::<T>(self).unwrap();
        chunks.push(new_chunk);
        Ok(cpu_local)
    }
}

impl<const ITEM_SIZE: usize> DynCpuLocalDealloc for CpuLocalAllocator<ITEM_SIZE> {
    fn dealloc(&self, vaddr: Vaddr) -> Option<()> {
        let mut chunks = self.chunks.lock();

        chunks
            .iter_mut()
            .position(|chunk| chunk.try_dealloc(vaddr).is_some())
            .map(|chunk_index| {
                // The item has already been deallocated.
                if chunks[chunk_index].is_empty()
                    && chunks.iter().filter(|c| c.is_empty()).count() > 1
                {
                    chunks.remove(chunk_index);
                }
            })
    }
}

/// Allocates a CPU-local object of type `T`.
pub fn alloc_cpu_local<T>() -> Result<CpuLocal<T, DynamicStorage<T>>> {
    let size = core::mem::size_of::<T>();
    let class = CommonSizeClass::from_size(size).ok_or(Error::InvalidArgs)?;
    match class {
        CommonSizeClass::Bytes8 => ALLOCATOR_8.alloc::<T>(),
        CommonSizeClass::Bytes16 => ALLOCATOR_16.alloc::<T>(),
        CommonSizeClass::Bytes32 => ALLOCATOR_32.alloc::<T>(),
        CommonSizeClass::Bytes64 => ALLOCATOR_64.alloc::<T>(),
        CommonSizeClass::Bytes128 => ALLOCATOR_128.alloc::<T>(),
        CommonSizeClass::Bytes256 => ALLOCATOR_256.alloc::<T>(),
        CommonSizeClass::Bytes512 => ALLOCATOR_512.alloc::<T>(),
        CommonSizeClass::Bytes1024 => ALLOCATOR_1024.alloc::<T>(),
        CommonSizeClass::Bytes2048 => ALLOCATOR_2048.alloc::<T>(),
    }
}
