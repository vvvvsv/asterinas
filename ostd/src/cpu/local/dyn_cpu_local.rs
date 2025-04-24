// SPDX-License-Identifier: MPL-2.0

//! Dynamically-allocated CPU-local objects.

use core::marker::PhantomData;

use align_ext::AlignExt;
use bitvec::prelude::{bitvec, BitVec};

use super::{AnyStorage, CpuLocal};
use crate::{
    cpu::{all_cpus, num_cpus, CpuId, PinCurrentCpu},
    mm::{paddr_to_vaddr, FrameAllocOptions, Segment, Vaddr, PAGE_SIZE},
    trap::DisabledLocalIrqGuard,
    Result,
};

/// A dynamic storage for a CPU-local variable of type `T`.
///
/// Such a CPU-local storage is not intended to be allocated directly.
/// Use `osdk_heap_allocator::alloc_cpu_local` instead.
pub struct DynamicStorage<T> {
    ptr: *const T,
    deallocator: &'static dyn DynCpuLocalDealloc,
}

impl<T> Drop for DynamicStorage<T> {
    fn drop(&mut self) {
        self.deallocator.dealloc(self.ptr as Vaddr).unwrap();
    }
}

unsafe impl<T> AnyStorage<T> for DynamicStorage<T> {
    fn get_ptr_on_current(&self, guard: &DisabledLocalIrqGuard) -> *const T {
        self.get_ptr_on_target(guard.current_cpu())
    }

    fn get_ptr_on_target(&self, cpu_id: CpuId) -> *const T {
        let bsp_va = self.ptr as usize;
        let va = bsp_va + cpu_id.as_usize() * CHUNK_SIZE;
        va as *const T
    }
}

impl<T> CpuLocal<T, DynamicStorage<T>> {
    /// Create a new dynamic CPU-local object.
    ///
    /// The given `ptr` points to the variable located on the BSP.
    /// The corresponding variables on all CPUs are initialized to zero.
    ///
    /// Please do not call this function directly. Instead, use
    /// `osdk_heap_allocator::alloc_cpu_local`.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the new per-CPU object belongs to an
    /// existing `DynCpuLocalChunk`, and does not overlap with any existing
    /// CPU-local object.
    #[doc(hidden)]
    pub unsafe fn __new_dynamic(
        ptr: *const T,
        deallocator: &'static dyn DynCpuLocalDealloc,
    ) -> Self {
        let storage = DynamicStorage { ptr, deallocator };
        for cpu in all_cpus() {
            let ptr = storage.get_ptr_on_target(cpu);
            // SAFETY: `ptr` points to the local variable of `storage` on `cpu`.
            unsafe {
                let mut_ptr = ptr as *mut T;
                core::ptr::write_bytes(mut_ptr, 0, 1);
            }
        }

        Self {
            storage,
            phantom: PhantomData,
        }
    }
}

const CHUNK_SIZE: usize = PAGE_SIZE;

/// Manages dynamic CPU-local chunks.
///
/// Each CPU owns a chunk of size `CHUNK_SIZE`, and the chunks are laid
/// out contiguously in the order of CPU IDs. Per-CPU variables lie within
/// the chunks.
pub struct DynCpuLocalChunk<const ITEM_SIZE: usize> {
    segment: Segment<()>,
    bitmap: BitVec,
}

impl<const ITEM_SIZE: usize> DynCpuLocalChunk<ITEM_SIZE> {
    /// Create a new dynamic CPU-local chunk.
    pub fn new() -> Result<Self> {
        let total_chunk_size = (CHUNK_SIZE * num_cpus()).align_up(PAGE_SIZE);
        let segment = FrameAllocOptions::new()
            .zeroed(false)
            .alloc_segment(total_chunk_size / PAGE_SIZE)?;

        let num_items = CHUNK_SIZE / ITEM_SIZE;
        debug_assert!(num_items * ITEM_SIZE == CHUNK_SIZE);
        Ok(Self {
            segment,
            bitmap: bitvec![0; num_items],
        })
    }

    /// Returns a pointer to the local chunk owned by the BSP.
    fn get_start_vaddr(&self) -> Vaddr {
        paddr_to_vaddr(self.segment.start_paddr())
    }

    /// Attempts to allocate a CPU-local object from the chunk.
    ///
    /// If the chunk is full, returns None.
    pub fn try_alloc<T>(
        &mut self,
        deallocator: &'static dyn DynCpuLocalDealloc,
    ) -> Option<CpuLocal<T, DynamicStorage<T>>> {
        debug_assert!(core::mem::size_of::<T>() <= ITEM_SIZE);
        let index = self.bitmap.first_zero()?;
        self.bitmap.set(index, true);
        // SAFETY: `index` refers to an available position in the chunk
        // for allocating a new CPU-local object.
        unsafe {
            let vaddr = self.get_start_vaddr() + index * ITEM_SIZE;
            Some(CpuLocal::__new_dynamic(vaddr as *const T, deallocator))
        }
    }

    /// Attempts to deallocate a previously allocated CPU-local object.
    ///
    /// If `vaddr` points to an item belonging to this chunk, deallocates it
    /// and returns `Some`; otherwise, returns `None`.
    pub fn try_dealloc(&mut self, vaddr: Vaddr) -> Option<()> {
        let start_vaddr = self.get_start_vaddr();
        let offset = vaddr.checked_sub(start_vaddr)?;
        if offset >= CHUNK_SIZE || offset % ITEM_SIZE != 0 {
            return None;
        }
        let index = offset / ITEM_SIZE;
        if self.bitmap[index] {
            self.bitmap.set(index, false);
            Some(())
        } else {
            None
        }
    }

    /// Checks whether the chunk is full.
    pub fn is_full(&self) -> bool {
        self.bitmap.all()
    }

    /// Checks whether the chunk is empty.
    pub fn is_empty(&self) -> bool {
        self.bitmap.not_any()
    }
}

/// A trait to dealloc CPU-local objects.
pub trait DynCpuLocalDealloc {
    /// Deallocates a previously allocated CPU-local object
    /// whose address is `vaddr`.
    fn dealloc(&self, vaddr: Vaddr) -> Option<()>;
}
