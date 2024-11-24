#!/usr/bin/env python3

import qmp
import sys

def onlinecpu():
    import numa

    o_cpus = []
    for node in range(0,numa.get_max_node()+1):
        for cpu in sorted(numa.node_to_cpus(node)):
            o_cpus.append(cpu)

    return o_cpus


def pin_proc(pid, core):
    import psutil

    try:
        psutil.Process(pid).cpu_affinity([core])
    except ValueError as e:
        print >> sys.stderr, e
        sys.exit(1)

def send_to_guest(query, cmd):
    # Map special characters to their corresponding key names
    def key_map(c):
        key_mapping = {
            '/': 'slash',
            '\\': 'backslash',
            ' ': 'spc',
            '\n': 'ret',
            '.': 'dot',
            '-': 'minus',
            '=': 'equal',
            '\t': 'tab',
            '\r': 'ret',
            ',': 'comma',
            ';': 'semicolon',
            '\'': 'apostrophe',
            '`': 'grave_accent',
            '[': 'bracket_left',
            ']': 'bracket_right',
        }
        if c in key_mapping:
            return key_mapping[c]
        return c

    # Add a return key at the end of the command
    cmd += '\n'

    # Replace special characters in the command
    keys = [ { "type": "qcode", "data": key_map(c) } for c in cmd ]

    send_command = {
        'execute': 'send-key',
        "arguments": { "keys": keys }
    }

    response = query.cmd_obj(send_command)
    print(response)

def main():
    # Arguments:
    # 1 --> port number
    # 2 --> number of vcpus
    # 3 --> the command send to the guest's shell

    if len(sys.argv) != 4:
        print("Usage: %s <port> <nr_vcpus> <cmd>" % sys.argv[0])
        sys.exit(1)

    port = sys.argv[1]
    nr_vcpus = sys.argv[2]
    cmd = sys.argv[3]

    query = qmp.QMPQuery("localhost:%s" % port)

    # Pin CPUs
    print(query.cmd("query-cpus-fast"))
    response = query.cmd("query-cpus-fast")['return']
    o_cpus = [x for x in range(int(nr_vcpus))]

    for i in range(int(nr_vcpus)):
        pin_proc(int(response[i]['thread-id']), o_cpus[i])

    # Send command to the guest's shell
    send_to_guest(query, cmd)

if __name__ == "__main__":
    main()
