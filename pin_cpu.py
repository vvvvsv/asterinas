#!/usr/bin/env python3

import qmp
import sys

def pin_proc(pid, core):
    import psutil

    try:
        psutil.Process(pid).cpu_affinity(list(core))
    except ValueError as e:
        print >> sys.stderr, e
        sys.exit(1)


# 1 --> port number
# 2 --> number of vcpus
query = qmp.QMPQuery("localhost:%s" % (sys.argv[1]))
# print(query.cmd("query-cpus-fast"))
response = query.cmd("query-cpus-fast")['return']
o_cpus = [x for x in range(int(sys.argv[2]))]
print("Number of vCPUs: ", len(response))

node0_cpus = range(0,192)
node1_cpus = range(192,384)

for i in range(int(sys.argv[2])):
    pid = int(response[i]['thread-id'])
    print("vCPU thread-id: {}, vCPU core-id: {}".format(pid, response[i]['props']['core-id']))

    if response[i]['props']['core-id'] < int(sys.argv[2])/2:
        host_cores = node0_cpus
    else:
        host_cores = node1_cpus

    print("Pinning process {} to host cores {}".format(pid, host_cores))
    pin_proc(pid, host_cores)