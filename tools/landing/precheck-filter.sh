# prechecked N...: prints the PRs whose `parity_squash.py precheck` says PASS, in order; every other line of precheck (REFUSED: names) goes to stderr, for the runner log.
# PRECHECK_CMD (default: parity_squash.py precheck in the landing checkout) is run with the PR number as its argument.
prechecked() {
  local n line
  for n in "$@"; do
    line=$(${PRECHECK_CMD:-precheck_default} "$n" 2>&1 | tail -1)
    case "$line" in *PASS) echo "$n";; *) echo "${line:-$n: REFUSED: precheck printed nothing}" >&2;; esac
  done
}
precheck_default() { (cd "$W" && uv run -q --project tools python tools/parity_squash.py precheck "$1"); }
