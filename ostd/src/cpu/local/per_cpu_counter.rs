use alloc::vec::Vec;
use core::{
    marker::PhantomData,
    mem::size_of,
    ops::Deref,
    sync::atomic::AtomicU64,
};

use align_ext::AlignExt;
use bitvec::prelude::{bitvec, BitVec};

use crate::{
    cpu::{num_cpus, CpuId, PinCurrentCpu},
    mm::{paddr_to_vaddr, FrameAllocOptions, Segment, Vaddr, PAGE_SIZE},
    sync::SpinLock,
    trap::DisabledLocalIrqGuard,
    Error, Result,
};

const PER_CPU_CHUNK_SIZE: usize = PAGE_SIZE;

pub struct PerCpuCounter<V> {
    ptr: *const V,
}

impl<V> PerCpuCounter<V> {
    const unsafe fn __new(ptr: *const V) -> Self {
        Self { ptr }
    }

    pub fn get_with<'a>(
        &'a self,
        guard: &'a DisabledLocalIrqGuard,
    ) -> PerCpuCounterDerefGuard<'a, V> {
        PerCpuCounterDerefGuard {
            per_cpu_counter: self,
            guard,
        }
    }

    pub(crate) unsafe fn as_ptr(&self, cpu_id: CpuId) -> *const V {
        let bsp_va = self.ptr as usize;
        let va = bsp_va + cpu_id.as_usize() * PER_CPU_CHUNK_SIZE;
        va as *const V
    }
}

impl<V: Sync> PerCpuCounter<V> {
    pub fn get_on_cpu(&self, cpu_id: CpuId) -> &V {
        unsafe { &*self.as_ptr(cpu_id) }
    }
}

unsafe impl<V: Sync> Sync for PerCpuCounter<V> {}
unsafe impl<V: Send> Send for PerCpuCounter<V> {}

impl<V> !Copy for PerCpuCounter<V> {}
impl<V> !Clone for PerCpuCounter<V> {}

#[must_use]
pub struct PerCpuCounterDerefGuard<'a, V> {
    per_cpu_counter: &'a PerCpuCounter<V>,
    guard: &'a DisabledLocalIrqGuard,
}

impl<V> Deref for PerCpuCounterDerefGuard<'_, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.per_cpu_counter.as_ptr(self.guard.current_cpu()) }
    }
}

struct PerCpuCrunk<T> {
    segment: Segment<()>,
    bitmap: BitVec,
    phantom: PhantomData<T>,
}

impl<T> PerCpuCrunk<T> {
    fn new() -> Result<Self> {
        let total_chunk_size = (PER_CPU_CHUNK_SIZE * num_cpus()).align_up(PAGE_SIZE);
        let segment = FrameAllocOptions::new()
            .zeroed(false)
            .alloc_segment(total_chunk_size / PAGE_SIZE)?;

        let num_items = PER_CPU_CHUNK_SIZE / size_of::<T>();
        Ok(Self {
            segment,
            bitmap: bitvec![0; num_items],
            phantom: PhantomData,
        })
    }

    fn get_area_ptr(&self) -> *const T {
        paddr_to_vaddr(self.segment.start_paddr()) as *const T
    }

    fn is_item_ptr<V>(&self, ptr: *const V) -> bool {
        let addr = ptr as Vaddr;
        let start_addr = self.get_area_ptr() as usize;
        start_addr <= addr
            && addr < start_addr + PER_CPU_CHUNK_SIZE
            && (addr - start_addr) % size_of::<T>() == 0
    }

    fn alloc<V>(&mut self) -> PerCpuCounter<V> {
        debug_assert!(!self.is_full());
        let index = self.bitmap.first_zero().unwrap();
        self.bitmap.set(index, true);
        unsafe {
            let area_ptr: *const T = self.get_area_ptr();
            PerCpuCounter::<V>::__new(area_ptr.add(index) as *const V)
        }
    }

    unsafe fn dealloc<V>(&mut self, value: PerCpuCounter<V>) {
        let index = unsafe {
            let ptr = value.ptr as *const T;
            let area_ptr: *const T = self.get_area_ptr();
            ptr.offset_from(area_ptr) as usize
        };
        self.bitmap.set(index, false);
    }

    fn is_full(&self) -> bool {
        self.bitmap.all()
    }

    fn is_empty(&self) -> bool {
        self.bitmap.not_any()
    }
}

pub struct PerCpuCounterAllocator {
    chunks: SpinLock<Vec<PerCpuCrunk<AtomicU64>>>,
}

pub static PER_CPU_COUNTER_ALLOCATOR: PerCpuCounterAllocator = PerCpuCounterAllocator::new();

impl PerCpuCounterAllocator {
    const fn new() -> Self {
        Self {
            chunks: SpinLock::new(Vec::new()),
        }
    }

    pub fn alloc<V>(&self) -> Result<PerCpuCounter<V>> {
        debug_assert!(size_of::<V>() <= size_of::<AtomicU64>());
        let mut chunks = self.chunks.lock();
        for chunk in chunks.iter_mut() {
            if !chunk.is_full() {
                return Ok(chunk.alloc::<V>());
            }
        }
        let mut new_chunk = PerCpuCrunk::new()?;
        let value = new_chunk.alloc::<V>();
        chunks.push(new_chunk);
        Ok(value)
    }

    pub fn dealloc<V>(&self, value: PerCpuCounter<V>) -> Result<()> {
        debug_assert!(size_of::<V>() <= size_of::<AtomicU64>());
        let mut chunks = self.chunks.lock();
        let chunk_index = chunks
            .iter_mut()
            .position(|chunk| chunk.is_item_ptr::<V>(value.ptr))
            .ok_or(Error::InvalidArgs)?;
        unsafe {
            chunks[chunk_index].dealloc(value);
        }
        if chunks[chunk_index].is_empty() {
            chunks.remove(chunk_index);
        }
        Ok(())
    }
}
