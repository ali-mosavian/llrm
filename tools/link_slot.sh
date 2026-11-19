#!/bin/sh
# Linker wrapper: at most LLRM_LINK_SLOTS (default 4) links run at once on the machine.
# Every test binary links the whole stack (~200 MB); dozens in parallel saturate the disk.
n=${LLRM_LINK_SLOTS:-4}
d=${TMPDIR:-/tmp}/llrm-link-slots-$(id -u)
mkdir -p "$d"
while :; do
    i=0
    while [ "$i" -lt "$n" ]; do
        exec 9>"$d/$i"
        if flock -n 9; then exec cc "$@"; fi
        i=$((i + 1))
    done
    sleep 1
done
