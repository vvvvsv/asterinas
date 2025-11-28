# Setup

Need a host with more than one NUMA nodes. Compile a `qemu-system-x86_64` with NUMA binding support according to the commit "Install qemu with NUMA support". Modify the file `tools/qemu_args.sh` according to the comments at lines 127-129 in that file.

## Test with NUMA-aware frame allocator

```
# (on branch `parse_numa`)

make run SMP=2 MEM=32G NUMA=true RELEASE=1 ENABLE_BASIC_TEST=true

# (in another terminal)
# (modify `node0_cpus` and `node1_cpus` in `pin_cpu.py` according to host NUMA nodes)
python3 pin_cpu.py 9889 2

# (back to qemu)
./test/test_numa.sh
```

## Test without NUMA-aware frame allocator

We cannot directly test on the main branch. Running the main branch in QEMU can still trigger first-touch NUMA-aware frame allocation. For example, if QEMU vCPU 8 is pinned to host CPU 192, when this vCPU allocates frames, the host will still assign pages from the node where host CPU 192 resides.

To better simulate the behavior on a physical machine, we need to use NUMA-related QEMU arguments to bind all of the memory of QEMU to host node 0.

```
# (on the commit "Add NUMA-related cargo feature, qemu args, and github workflow")
# (cherry-pick the last commit `Some tests`)
# (modify the line `HOST_NODE1=$((HOST_NUMA_NODES<2?0:1))` in `tools/qemu_args.sh` to `HOST_NODE1=0`)

# (run the same commands as the last section)
```


# Results

The results below are evaluated on a 2-socket AMD EPYC 9965 machine, each socket has a 192-core CPU and 512GB DRAM, showing a **7.88%** speedup.

## Test with NUMA-aware frame allocator

```
~ # ./test/test_numa.sh
Run #1:
Time elapsed: 9.149 s
Run #2:
Time elapsed: 8.950 s
Run #3:
Time elapsed: 8.542 s
Run #4:
Time elapsed: 8.981 s
Run #5:
Time elapsed: 8.850 s
Run #6:
Time elapsed: 9.611 s
Run #7:
Time elapsed: 9.572 s
Run #8:
Time elapsed: 9.757 s
Run #9:
Time elapsed: 9.150 s
Run #10:
Time elapsed: 9.118 s
Run #11:
Time elapsed: 9.524 s
Run #12:
Time elapsed: 8.713 s
Run #13:
Time elapsed: 9.345 s
Run #14:
Time elapsed: 9.046 s
Run #15:
Time elapsed: 9.051 s
Run #16:
Time elapsed: 10.058 s
Run #17:
Time elapsed: 8.740 s
Run #18:
Time elapsed: 9.266 s
Run #19:
Time elapsed: 9.550 s
Run #20:
Time elapsed: 9.047 s
Average Time elapsed over 20 runs: 9.201 s
```

## Test without NUMA-aware frame allocator

```
~ # ./test/test_numa.sh
Run #1:
Time elapsed: 9.362 s
Run #2:
Time elapsed: 9.963 s
Run #3:
Time elapsed: 8.672 s
Run #4:
Time elapsed: 9.138 s
Run #5:
Time elapsed: 9.298 s
Run #6:
Time elapsed: 8.827 s
Run #7:
Time elapsed: 8.833 s
Run #8:
Time elapsed: 10.458 s
Run #9:
Time elapsed: 10.170 s
Run #10:
Time elapsed: 10.417 s
Run #11:
Time elapsed: 9.502 s
Run #12:
Time elapsed: 10.010 s
Run #13:
Time elapsed: 10.125 s
Run #14:
Time elapsed: 9.907 s
Run #15:
Time elapsed: 10.143 s
Run #16:
Time elapsed: 10.313 s
Run #17:
Time elapsed: 10.368 s
Run #18:
Time elapsed: 11.712 s
Run #19:
Time elapsed: 11.376 s
Run #20:
Time elapsed: 9.932 s
Average Time elapsed over 20 runs: 9.926 s
```
