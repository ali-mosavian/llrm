# stamp_lines: each stdin line prefixed with the time it ARRIVED, flushed at once. mawk (this host's awk) reads a pipe in blocks: the batch log stayed empty for 34 minutes
# and showed one time on every line.
stamp_lines() {
  local line
  while IFS= read -r line || [ -n "$line" ]; do printf '%s %s\n' "$(date +%H:%M:%S)" "$line"; done
}
