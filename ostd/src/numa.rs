// SPDX-License-Identifier: MPL-2.0

//! NUMA(Non Uniform Memory Access) support.

use spin::Once;

use crate::{
    arch::boot::numa::{
        distance, init_numa_topology, is_numa, num_nodes, MEMORY_RANGES, PROCESSOR_AFFINITIES,
    },
    cpu::{all_cpus, num_cpus, CpuId},
    cpu_local,
};

pub(super) fn init() {
    init_numa_topology();

    if is_numa() {
        // PRINT START
        let num_numa_nodes = num_nodes();
        log::warn!("num_numa_nodes: {}", num_numa_nodes);
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
        for i in 0..num_numa_nodes {
            for j in 0..num_numa_nodes {
                log::warn!("distance {}->{}: {}", i, j, distance(i, j).unwrap());
            }
        }
        // PRINT END

        for affinitiy in PROCESSOR_AFFINITIES.get().unwrap().iter() {
            if !affinitiy.is_enabled {
                continue;
            }
            let node_id = affinitiy.proximity_domain;
            let cpu_id = CpuId::try_from(affinitiy.local_apic_id as usize).unwrap();
            NODE_ID.get_on_cpu(cpu_id).call_once(|| node_id);
        }
        for cpu_id in all_cpus() {
            NODE_ID.get_on_cpu(cpu_id).call_once(|| 0);
        }

        // Initialize the leader CPU of each CPU. Since the number of CPUs won't be too large,
        // and `alloc` is not allowed now, the O(n^2) approach is suitable here.
        for cpu_id in all_cpus() {
            let leader_cpu = LEADER_CPU.get_on_cpu(cpu_id);
            if leader_cpu.is_completed() {
                continue;
            }
            leader_cpu.call_once(|| cpu_id);
            let node_id = NODE_ID.get_on_cpu(cpu_id).get().unwrap();
            let mut num_cpus_in_this_node = 1;

            all_cpus()
                .filter(|&id| {
                    id.as_usize() > cpu_id.as_usize()
                        && NODE_ID.get_on_cpu(id).get().unwrap() == node_id
                })
                .for_each(|non_leader_id| {
                    debug_assert!(!LEADER_CPU.get_on_cpu(non_leader_id).is_completed());
                    num_cpus_in_this_node += 1;
                    LEADER_CPU.get_on_cpu(non_leader_id).call_once(|| cpu_id);
                });
            num_cpus_in_node(cpu_id).call_once(|| num_cpus_in_this_node);
        }

        // FIXME: Add a fall back list for each NUMA node.
    } else {
        for cpu_id in all_cpus() {
            NODE_ID.get_on_cpu(cpu_id).call_once(|| 0);
            LEADER_CPU.get_on_cpu(cpu_id).call_once(|| CpuId::bsp());
        }
        num_cpus_in_node(CpuId::bsp()).call_once(|| num_cpus());
    }
}

cpu_local! {
    /// The NUMA node ID of the current CPU.
    pub static NODE_ID: Once<u32> = Once::new();
    /// The leader CPU of the current CPU, i.e., the CPU with the smallest ID
    /// in the current CPU's NUMA node.
    pub static LEADER_CPU: Once<CpuId> = Once::new();
}

/// Returns whether the given CPU is the leader CPU of its NUMA node.
pub fn is_leader_cpu(cpu_id: CpuId) -> bool {
    LEADER_CPU.get_on_cpu(cpu_id).get().unwrap() == &cpu_id
}

/// Returns the leader CPU of the NUMA node of the given CPU.
pub fn leader_cpu_of(cpu_id: CpuId) -> CpuId {
    LEADER_CPU.get_on_cpu(cpu_id).get().unwrap().clone()
}

/// Defines a statically-allocated CPU-local variable, which is only meaningful
/// on leader CPUs, and automatically generates accessor functions for it.
///
/// # Examples
///
/// ```rust
/// leader_cpu_local! {
///     pub static FOO: AtomicU32 = AtomicU32::new(1);
/// }
/// ```
///
/// The above will be expanded to:
///
/// ```rust
/// cpu_local! {
///     static FOO: AtomicU32 = AtomicU32::new(1);
/// }
///
/// pub fn foo(leader_cpu: CpuId) -> &'static AtomicU32 {
///   debug_assert!(is_leader_cpu(leader_cpu));
///   FOO.get_on_cpu(leader_cpu)
/// }
/// ```
///
/// # Panics
///
/// The accessor functions panic if the given CPU is not a leader CPU.
#[macro_export]
macro_rules! leader_cpu_local {
    ($( $(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr; )*) => {
        cpu_local! {
            $(
                $(#[$attr])*
                static $name: $t = $init;
            )*
        }

        $(
            paste::paste! {
                $(#[$attr])*
                $vis fn [<$name:lower>](leader_cpu: CpuId) -> &'static $t {
                    debug_assert!(is_leader_cpu(leader_cpu));
                    $name.get_on_cpu(leader_cpu)
                }
            }
        )*
    };
}

leader_cpu_local! {
    /// The number of CPUs in the NUMA node of the leader CPU.
    pub static NUM_CPUS_IN_NODE: Once<usize> = Once::new();
}
