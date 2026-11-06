# disk_pause [DIR...]: waits while DIR (default /tmp and $LAND_TMP) has under DISK_MIN_FREE_PCT (default 10) percent of its blocks or inodes free.
# /tmp reached 99% of its inodes and a batch failed its links with "Cannot create temporary file".
disk_low() {
  local dir kind used
  for dir in "$@"; do
    for kind in "-P" "-Pi"; do
      used=$(df $kind "$dir" 2>/dev/null | awk 'NR==2{gsub("%","",$5); print $5}')
      [ -n "$used" ] && [ $((100 - used)) -lt "${DISK_MIN_FREE_PCT:-10}" ] && { echo "PREFLIGHT: $dir has $((100 - used))% free ($kind): waiting"; return 0; }
    done
  done
  return 1
}
disk_pause() { [ $# -gt 0 ] || set -- /tmp "${LAND_TMP:-/tmp/landing}"; while disk_low "$@"; do sleep 60; done; }
