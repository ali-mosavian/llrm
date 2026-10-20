#!/usr/bin/env bash
# check-kills.sh FILE...: prints `file:line: text` for every line that ends a process by a name or pattern: pkill, killall, `xargs ... kill`, a `kill` whose target is
# a pgrep/pidof or a `$(...)` that is not `$(cat file)` (a recorded pid/pgid). A command line is not an identity: such a kill took down other sessions' work. Exit 1 when any line is flagged.
status=0
for f in "$@"; do
  out=$(awk -v f="$f" '
    { line=$0; sub(/^[ \t]*#.*/, "", line); if (line ~ /^[ \t]*$/) next
      t=line; gsub(/[;|&()`"\047{}]/, " ", t); n=split(t, tok, /[ \t]+/); bad=0
      for (i=1;i<=n;i++) {
        if (tok[i]=="pkill" || tok[i]=="killall") bad=1
        if (tok[i]=="kill" || tok[i] ~ /^kill$/) {
          if (line ~ /xargs/) bad=1
          if (line ~ /pgrep|pidof/) bad=1
          if (match(line, /kill[^$]*\$\(/)) { rest=substr(line, RSTART+RLENGTH); if (rest !~ /^[ \t]*cat[ \t]/) bad=1 }
        }
      }
      if (bad) printf "%s:%d: %s\n", f, NR, $0 }' "$f")
  [ -n "$out" ] && { echo "$out"; status=1; }
done
exit $status
