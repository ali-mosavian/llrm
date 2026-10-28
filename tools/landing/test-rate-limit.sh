#!/usr/bin/env bash
# Regression: the quota ran out at 09:2x and every gh call became a 403: the queue went empty, PRs read as "not open", a squash was refused as the PR's own. With a spent quota the runner sleeps until the reset.
here=$(cd "$(dirname "$0")" && pwd); source "$here/rate-limit.sh"; d=$(mktemp -d)
reset=$(( $(date +%s) + 600 ))
printf '#!/bin/sh\necho "8 %s"\n' "$reset" > $d/gh; chmod +x $d/gh; PATH=$d:$PATH
slept=""; sleep() { slept="$slept $1"; }
rate_limit_pause 2>/dev/null
[ "${slept# }" -ge 600 ] 2>/dev/null && [ "${slept# }" -le 610 ] || { echo "FAIL: 8 calls left should sleep until the reset (~602 s): '$slept'"; exit 1; }
slept=""; printf '#!/bin/sh\necho "4000 %s"\n' "$reset" > $d/gh; rate_limit_pause 2>/dev/null
[ -z "$slept" ] || { echo "FAIL: 4000 calls left should not sleep: '$slept'"; exit 1; }
slept=""; printf '#!/bin/sh\nexit 1\n' > $d/gh; rate_limit_pause 2>/dev/null
[ -z "$slept" ] || { echo "FAIL: an unreadable quota should not sleep: '$slept'"; exit 1; }
echo PASS
