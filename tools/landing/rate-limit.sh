# rate_limit_pause: sleeps until the core quota resets when fewer than RATE_LIMIT_MIN (default 100) calls are left (`gh api rate_limit` costs none), so a spent quota pauses the runner
# instead of turning every gh call into a 403 (empty queue, PRs "not open", refusals).
rate_limit_pause() {
  local left reset wait
  read -r left reset < <(gh api rate_limit -q '.resources.core | "\(.remaining) \(.reset)"' 2>/dev/null) || return 0
  [[ $left =~ ^[0-9]+$ && $reset =~ ^[0-9]+$ ]] || return 0
  [ "$left" -ge "${RATE_LIMIT_MIN:-100}" ] && return 0
  wait=$(( reset - $(date +%s) + 2 )); [ "$wait" -lt 5 ] && wait=5; [ "$wait" -gt 3700 ] && wait=3700
  echo "gh rate limit spent ($left left); pausing ${wait}s for the reset" >&2
  sleep "$wait"
}
