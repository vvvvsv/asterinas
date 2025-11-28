#!/bin/sh

total=0
count=20

for i in $(seq 1 $count); do
    echo "Run #$i:"
    output=$(./test/mmap/test_numa)
    echo "$output"
    time=$(echo "$output" | sed -n 's/Time elapsed: \([0-9.]*\) s/\1/p')
    total=$(echo "$total + $time" | bc)
done

average=$(echo "scale=3; $total / $count" | bc)
echo "Average Time elapsed over $count runs: $average s"
