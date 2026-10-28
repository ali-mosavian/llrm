#!/usr/bin/env bash
# Regression: #2919 landed by a hand merge, absent from landed.log, and was found by chance. A first-parent commit not in landed.log is reported with its PR and owner;
# logged ones are not.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d); export UNLOGGED_CACHE=$d/cache0
git init -q $d/r && cd $d/r && git config user.email t@t && git config user.name t
for n in one two three; do echo $n > f; git add f; git commit -q -m "feat: $n"; done
a=$(git rev-parse HEAD~2); b=$(git rev-parse HEAD~1); c=$(git rev-parse HEAD)
printf '%s #1 gate-ok 2026-10-19\n%s #2 gate-ok 2026-10-19\n' $a $b > $d/landed.log
mkdir $d/bin; printf '#!/bin/sh\ncase "$1" in api) echo 77;; pr) echo "Owner: some-session";; esac\n' > $d/bin/gh; chmod +x $d/bin/gh
out=$(PATH=$d/bin:$PATH LANDED_LOG=$d/landed.log "$here/unlogged.sh" HEAD); rc=$?
[ $rc = 1 ] && echo "$out" | grep -q "${c:0:9} #77 owner some-session: feat: three" || { echo "FAIL: unlogged commit not reported (rc=$rc): $out"; exit 1; }
echo "$out" | grep -q "${a:0:9}\|${b:0:9}" && { echo "FAIL: a logged commit was reported"; exit 1; }
echo "$c #3 gate-ok 2026-10-19" >> $d/landed.log
PATH=$d/bin:$PATH LANDED_LOG=$d/landed.log "$here/unlogged.sh" HEAD >/dev/null || { echo "FAIL: all logged but still reported"; exit 1; }
# a commit seen once is not asked about again: the runner ran this every cycle and spent ~2 min on gh calls for the same commits
printf '#!/bin/sh\necho "gh called again" >&2; exit 1\n' > $d/bin/gh
out=$(PATH=$d/bin:$PATH LANDED_LOG=$d/landed.log UNLOGGED_CACHE=$d/cache "$here/unlogged.sh" HEAD 2>&1)
rm -f $d/cache; printf '#!/bin/sh\ncase "$1" in api) echo 77;; pr) echo "Owner: some-session";; esac\n' > $d/bin/gh
: > $d/landed.log
PATH=$d/bin:$PATH LANDED_LOG=$d/landed.log UNLOGGED_CACHE=$d/cache "$here/unlogged.sh" HEAD >/dev/null
printf '#!/bin/sh\necho "gh called again" >&2; exit 1\n' > $d/bin/gh
out=$(PATH=$d/bin:$PATH LANDED_LOG=$d/landed.log UNLOGGED_CACHE=$d/cache "$here/unlogged.sh" HEAD 2>&1)
echo "$out" | grep -q "#77 owner some-session: feat: three" && ! echo "$out" | grep -q "gh called again" || { echo "FAIL: a cached commit asked GitHub again: $out"; exit 1; }
echo PASS
