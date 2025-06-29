// SPDX-License-Identifier: MPL-2.0

//! NUMA(Non Uniform Memory Access) support.

use spin::Once;

use crate::{
    arch::boot::numa::{init_numa_topology, DISTANCE_MATRIX, MEMORY_RANGES, PROCESSOR_AFFINITIES},
    cpu::{all_cpus, CpuId},
    cpu_local,
    util::id_set::Id,
};

/// The number of NUMA nodes.
static mut NUM_NODES: usize = 0;

/// Initializes the number of NUMA nodes.
///
/// # Safety
///
/// The caller must ensure that we're in the boot context, and this method is
/// called only once.
unsafe fn init_num_nodes(num_nodes: usize) {
    assert!(num_nodes >= 1);

    // SAFETY: It is safe to mutate this global variable because we
    // are in the boot context.
    unsafe { NUM_NODES = num_nodes };
}

/// Returns the number of NUMA nodes.
pub fn num_nodes() -> usize {
    // SAFETY: As far as the safe APIs are concerned, `NUM_NODES` is initialized
    // and read-only, so it is always valid to read.
    unsafe { NUM_NODES }
}

cpu_local! {
    /// The NUMA node ID of the current CPU.
    static NODE_ID: Once<NodeId> = Once::new();
}

/// Returns the NUMA node ID of the given CPU.
pub fn node_id(cpu_id: CpuId) -> NodeId {
    *NODE_ID.get_on_cpu(cpu_id).get().unwrap()
}

pub(super) fn init() {
    let (num_nodes, leader_cpu) = init_numa_topology();

    // SAFETY: We're in the boot context, calling the method only once.
    unsafe { init_num_nodes(num_nodes) };

    for affinitiy in PROCESSOR_AFFINITIES.get().unwrap().iter() {
        if !affinitiy.is_enabled {
            continue;
        }
        let node_id = affinitiy.proximity_domain;
        let cpu_id = CpuId::try_from(affinitiy.local_apic_id as usize).unwrap();
        NODE_ID
            .get_on_cpu(cpu_id)
            .call_once(|| NodeId::new(node_id));
    }
    for cpu_id in all_cpus() {
        NODE_ID.get_on_cpu(cpu_id).call_once(|| NodeId::new(0));
    }

    some_print();
}

/// The ID of a NUMA node in the system.
///
/// If converting from/to an integer, the integer must start from 0 and be less
/// than the number of NUMA nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeId(u32);

impl NodeId {
    /// Creates a new instance.
    ///
    /// # Panics
    ///
    /// The given number must be smaller than the total number of NUMA nodes
    /// (`ostd::numa::num_nodes()`).
    pub fn new(raw_id: u32) -> Self {
        assert!(raw_id < num_nodes() as u32);
        // SAFETY: The raw ID is smaller than `num_nodes()`.
        unsafe { Self::new_unchecked(raw_id) }
    }
}

impl From<NodeId> for u32 {
    fn from(node_id: NodeId) -> Self {
        node_id.0
    }
}

// SAFETY: `NodeId`s and the integers within 0 to `num_nodes` (exclusive) have 1:1 mapping.
unsafe impl Id for NodeId {
    unsafe fn new_unchecked(raw_id: u32) -> Self {
        Self(raw_id)
    }

    fn cardinality() -> u32 {
        num_nodes() as u32
    }
}

fn some_print() {
    let num_nodes = num_nodes();
    log::warn!("num_nodes: {}", num_nodes);
    for processor_affinity in PROCESSOR_AFFINITIES.get().unwrap().iter() {
        log::warn!("processor_affinity: {:?}", processor_affinity);
    }
    for memory_range in MEMORY_RANGES.get().unwrap().iter() {
        log::warn!("memory_range: {:?}", memory_range);
    }
    // let regions: &crate::boot::memory_region::MemoryRegionArray<512> = &crate::boot::EARLY_INFO.get().unwrap().memory_regions;
    // for region in regions.iter() {
    //     if region.typ() == crate::boot::memory_region::MemoryRegionType::Usable {
    //         log::warn!("region: {:?}", region);
    //     }
    // }
    for cpu in all_cpus() {
        log::warn!(
            "cpu {}: node_id {}",
            cpu.as_usize(),
            node_id(cpu).as_usize()
        );
    }
    for i in 0..num_nodes {
        for j in 0..num_nodes {
            let dist = DISTANCE_MATRIX.get().unwrap()[i * num_nodes + j];
            log::warn!("distance {}->{}: {}", i, j, dist);
        }
    }
}
