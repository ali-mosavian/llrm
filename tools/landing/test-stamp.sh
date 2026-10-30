#!/usr/bin/env bash
# Regression: the batch log went through mawk, which buffers a pipe: nothing appeared until the batch ended and every line carried the same time.
. "$(dirname "$0")/stamp.sh"; d=$(mktemp -d)
( (echo first; sleep 3; echo second) | stamp_lines > $d/log ) &
sleep 1
[ "$(wc -l < $d/log)" = 1 ] && grep -q ' first$' $d/log || { echo "FAIL: after 1 s the log is '$(cat $d/log)'"; exit 1; }
wait; [ "$(wc -l < $d/log)" = 2 ] || { echo "FAIL: two lines expected"; exit 1; }
a=$(sed -n 1p $d/log | cut -d' ' -f1); b=$(sed -n 2p $d/log | cut -d' ' -f1)
[ "$a" != "$b" ] || { echo "FAIL: both lines carry $a"; exit 1; }
printf 'last without newline' | stamp_lines | grep -q 'last without newline' || { echo "FAIL: a last line without newline is lost"; exit 1; }
echo PASS
