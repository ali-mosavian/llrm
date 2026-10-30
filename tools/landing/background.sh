# background_timeout SECONDS LOG CMD...: CMD in the background under `timeout`, its output appended to LOG with the time; returns at once, so the runner never waits for it.
# sweep-targets.sh ran inline: 17 minutes at 19:03 and 19:43 before a batch could start.
background_timeout() {
  local seconds=$1 log=$2; shift 2
  ( timeout "$seconds" "$@" 2>&1 | sed "s/^/$(date +%H:%M) /" >> "$log" ) > /dev/null 2>&1 &
}
