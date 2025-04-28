// SPDX-License-Identifier: MPL-2.0

use crate::allocator::CommonSizeClass;
use ostd::{
    cpu::local::{CpuLocal, CpuLocalAllocator, DynamicStorage},
    prelude::*,
    Error,
};

/// Global allocators for dynamically-allocated CPU-Local objects.
static ALLOCATOR_8: CpuLocalAllocator<8> = CpuLocalAllocator::new();
static ALLOCATOR_16: CpuLocalAllocator<16> = CpuLocalAllocator::new();
static ALLOCATOR_32: CpuLocalAllocator<32> = CpuLocalAllocator::new();
static ALLOCATOR_64: CpuLocalAllocator<64> = CpuLocalAllocator::new();
static ALLOCATOR_128: CpuLocalAllocator<128> = CpuLocalAllocator::new();
static ALLOCATOR_256: CpuLocalAllocator<256> = CpuLocalAllocator::new();
static ALLOCATOR_512: CpuLocalAllocator<512> = CpuLocalAllocator::new();
static ALLOCATOR_1024: CpuLocalAllocator<1024> = CpuLocalAllocator::new();
static ALLOCATOR_2048: CpuLocalAllocator<2048> = CpuLocalAllocator::new();

/// Allocates a dynamically-allocated CPU-local object of type `T`.
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

pub fn dealloc_cpu_local<T>(cpu_local: CpuLocal<T, DynamicStorage<T>>) {
    let size = core::mem::size_of::<T>();
    let class = CommonSizeClass::from_size(size).unwrap();
    match class {
        CommonSizeClass::Bytes8 => ALLOCATOR_8.dealloc(cpu_local),
        CommonSizeClass::Bytes16 => ALLOCATOR_16.dealloc(cpu_local),
        CommonSizeClass::Bytes32 => ALLOCATOR_32.dealloc(cpu_local),
        CommonSizeClass::Bytes64 => ALLOCATOR_64.dealloc(cpu_local),
        CommonSizeClass::Bytes128 => ALLOCATOR_128.dealloc(cpu_local),
        CommonSizeClass::Bytes256 => ALLOCATOR_256.dealloc(cpu_local),
        CommonSizeClass::Bytes512 => ALLOCATOR_512.dealloc(cpu_local),
        CommonSizeClass::Bytes1024 => ALLOCATOR_1024.dealloc(cpu_local),
        CommonSizeClass::Bytes2048 => ALLOCATOR_2048.dealloc(cpu_local),
    }
}
