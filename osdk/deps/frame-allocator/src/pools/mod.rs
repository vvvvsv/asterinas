// SPDX-License-Identifier: MPL-2.0

mod balancing;

use core::{
    alloc::Layout,
    cell::RefCell,
    ops::{DerefMut, Drop, Range},
    sync::atomic::{AtomicUsize, Ordering},
};

use ostd::{
    arch::boot::numa::{is_numa, MEMORY_RANGES},
    cpu::{all_cpus, CpuId, PinCurrentCpu},
    cpu_local, cpu_local,
    irq::DisabledLocalIrqGuard,
    leader_cpu_local,
    mm::Paddr,
    numa::{is_leader_cpu, leader_cpu_of, num_cpus_in_node},
    sync::{LocalIrqDisabled, SpinLock, SpinLockGuard},
    util::ops::{get_intersected_range, is_intersected, range_difference},
};

use crate::chunk::{greater_order_of, lesser_order_of, size_of_order, split_to_chunks, BuddyOrder};

use super::set::BuddySet;

// The NUMA node pools.
leader_cpu_local! {
    /// Free buddies in the NUMA node of the leader CPU.
    static NODE_POOL: SpinLock<BuddySet<MAX_BUDDY_ORDER>, LocalIrqDisabled> = SpinLock::new(BuddySet::new_empty());
    /// A snapshot of the total size of the free buddies in the NUMA node of the leader CPU, not precise.
    static NODE_POOL_SIZE: AtomicUsize = AtomicUsize::new(0);
}

cpu_local! {
    /// CPU-local free buddies.
    static LOCAL_POOL: RefCell<BuddySet<MAX_LOCAL_BUDDY_ORDER>> = RefCell::new(BuddySet::new_empty());
}

/// Maximum supported order of the buddy system.
///
/// i.e., it is the number of classes of free blocks. It determines the
/// maximum size of each allocation.
///
/// A maximum buddy order of 32 supports up to 4KiB*2^31 = 8 TiB of chunks.
const MAX_BUDDY_ORDER: BuddyOrder = 32;

/// Maximum supported order of the buddy system for CPU-local buddy system.
///
/// Since large blocks are rarely allocated, caching such blocks will lead
/// to much fragmentation.
///
/// Lock guards are also allocated on stack. We can limit the stack usage
/// for common paths in this way.
///
/// A maximum local buddy order of 18 supports up to 4KiB*2^17 = 512 MiB of
/// chunks.
const MAX_LOCAL_BUDDY_ORDER: BuddyOrder = 18;

pub(super) fn alloc(guard: &DisabledLocalIrqGuard, layout: Layout) -> Option<Paddr> {
    let local_pool_cell = LOCAL_POOL.get_with(guard);
    let mut local_pool = local_pool_cell.borrow_mut();
    let leader_cpu = leader_cpu_of(guard.current_cpu());
    let mut node_pool = OnDemandNodeLock::new(leader_cpu);

    let size_order = greater_order_of(layout.size());
    let align_order = greater_order_of(layout.align());
    let order = size_order.max(align_order);

    let mut chunk_addr = None;

    if order < MAX_LOCAL_BUDDY_ORDER {
        chunk_addr = local_pool.alloc_chunk(order);
    }

    // Fall back to the NUMA node's free lists if the local free lists are empty.
    if chunk_addr.is_none() {
        chunk_addr = node_pool.get().alloc_chunk(order);
    }

    // TODO: On memory pressure the NUMA node pool may be not enough. We may need
    // to merge all buddy chunks from the local pools to the NUMA node pool and
    // try again.

    // FIXME: Fall back to the other NUMA node's free lists if the current NUMA node's
    // free lists are empty.

    // TODO: On memory pressure all the NUMA node pools may be not enough. We may need
    // to alloc accross NUMA nodes.

    // If the alignment order is larger than the size order, we need to split
    // the chunk and return the rest part back to the free lists.
    let allocated_size = size_of_order(order);
    if allocated_size > layout.size() {
        if let Some(chunk_addr) = chunk_addr {
            do_dealloc(
                &mut local_pool,
                &mut node_pool,
                [(chunk_addr + layout.size(), allocated_size - layout.size())].into_iter(),
            );
        }
    }

    balancing::balance(local_pool.deref_mut(), &mut node_pool);

    chunk_addr
}

pub(super) fn dealloc(
    guard: &DisabledLocalIrqGuard,
    segments: impl Iterator<Item = (Paddr, usize)>,
) {
    // 要在MetaSlot里面加一个NODEID吧
    let local_pool_cell = LOCAL_POOL.get_with(guard);
    let mut local_pool = local_pool_cell.borrow_mut();
    let mut global_pool = OnDemandNodeLock::new();

    do_dealloc(&mut local_pool, &mut global_pool, segments);

    balancing::balance(local_pool.deref_mut(), &mut global_pool);
}

pub(super) fn add_free_memory(_guard: &DisabledLocalIrqGuard, addr: Paddr, size: usize) {
    if is_numa() {
        let memory_ranges = MEMORY_RANGES.get().unwrap();
        let free_memory = addr..addr + size;

        for range in memory_ranges
            .iter()
            .filter(|range| range.is_enabled && range.proximity_domain.is_some())
        {
            let node_id = range.proximity_domain.unwrap();
            // 需要增加一个从NODEID到leader CPU的映射，加在parse silt那里。
            let leader_cpu = all_cpus()
                .find(|&cpu| NODE_ID.get_on_cpu(cpu).get().unwrap() == node_id)
                .unwrap_or(CpuId::bsp());
            // 找到addr..size和range的交集，利用range有序不交
        }
        // 找到剩下的addr..size和任何memory_ranges不交的部分

        let mut global_pool = OnDemandNodeLock::new();

        split_to_chunks(addr, size).for_each(|(addr, order)| {
            global_pool.get().insert_chunk(addr, order);
        });
    } else {
        let mut node_pool = OnDemandNodeLock::new(CpuId::bsp());

        split_to_chunks(addr, size).for_each(|(addr, order)| {
            node_pool.get().insert_chunk(addr, order);
        });
    }
}

fn do_dealloc(
    local_pool: &mut BuddySet<MAX_LOCAL_BUDDY_ORDER>,
    node_pool: &mut OnDemandNodeLock,
    segments: impl Iterator<Item = (Paddr, usize)>,
) {
    // 怎么把segment分成多端，分到各个numa node的pool里？
    segments.for_each(|(addr, size)| {
        split_to_chunks(addr, size).for_each(|(addr, order)| {
            if order >= MAX_LOCAL_BUDDY_ORDER {
                node_pool.get().insert_chunk(addr, order);
            } else {
                local_pool.insert_chunk(addr, order);
            }
        });
    });
}

type NodeLockGuard = SpinLockGuard<'static, BuddySet<MAX_BUDDY_ORDER>, LocalIrqDisabled>;

/// An on-demand guard that locks the NUMA node pool when needed.
///
/// It helps to avoid unnecessarily locking the node pool, and also avoids
/// repeatedly locking the node pool when it is needed multiple times.
struct OnDemandNodeLock {
    leader_cpu: CpuId,
    guard: Option<NodeLockGuard>,
}

impl OnDemandNodeLock {
    fn new(leader_cpu: CpuId) -> Self {
        Self {
            leader_cpu,
            guard: None,
        }
    }

    fn get(&mut self) -> &mut NodeLockGuard {
        self.guard
            .get_or_insert_with(|| node_pool(self.leader_cpu).lock())
    }

    /// Returns the size of the NUMA node pool.
    ///
    /// If the node pool is locked, returns the actual size of the node pool.
    /// Otherwise, returns the last snapshot of the node pool size by loading
    /// [`NODE_POOL_SIZE`].
    fn get_node_size(&self) -> usize {
        if let Some(guard) = self.guard.as_ref() {
            guard.total_size()
        } else {
            node_pool_size(self.leader_cpu).load(Ordering::Relaxed)
        }
    }

    /// Returns the number of CPUs in the NUMA node of the leader CPU.
    fn get_num_cpus_in_node(&self) -> usize {
        num_cpus_in_node(self.leader_cpu).get().unwrap().clone()
    }
}

impl Drop for OnDemandNodeLock {
    fn drop(&mut self) {
        // Updates the [`NODE_POOL_SIZE`] if the node pool is locked.
        if let Some(guard) = self.guard.as_ref() {
            node_pool_size(self.leader_cpu).store(guard.total_size(), Ordering::Relaxed);
        }
    }
}
