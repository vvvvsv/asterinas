set +m

echo start

( sleep 1 ) &
am_sleep_pid=$!

echo hello

if test -n "$am_sleep_pid"; then
    echo "waiting for sleep to finish"
    wait "$am_sleep_pid"
    echo "sleep finished"
fi

echo end
