// SPDX-License-Identifier: MPL-2.0

//! NUMA(Non Uniform Memory Access) support.

use crate::arch::boot::numa::{
    distance, init_numa_topology, is_numa, num_nodes, MEMORY_RANGES, PROCESSOR_AFFINITIES,
};

pub(super) fn init() {
    init_numa_topology();

    if is_numa() {
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
    }
}
