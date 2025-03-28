#!/bin/bash

tmux new-session -d -s aster_session

tmux send-keys -t aster_session:0 'make run SMP=64 2>&1 | tee aster_output.txt' Enter

sleep 40

tmux new-window -t aster_session:1

tmux send-keys -t aster_session:1 'python3 tools/pin-cpu.py 9889 64' Enter

sleep 5

tmux select-window -t aster_session:0

tmux send-keys -t aster_session:0 './test/microbench-vm-scale.sh' Enter