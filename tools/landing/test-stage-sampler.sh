#!/usr/bin/env bash
# Regression: the sampler grepped the whole host, so another session's `cargo build --profile dist` was logged as the landing's build-dist (5 min 12 s, reported to oversight as fact).
# A cargo build that is not a descendant of parity_squash must not appear; the old host-wide grep must fail this test.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d); tag="psq$RANDOM$$"
printf '#!/bin/bash\nbash -c "sleep 8; : # cargo build --release" &\nwait\n' > "$d/$tag.py"
setsid bash "$d/$tag.py" merge-batch 1 2 >/dev/null 2>&1 &
echo $! > "$d/root.pid"

setsid bash -c "sleep 8; : # cargo build --profile dist" >/dev/null 2>&1 &

sleep 1.5
export ROOT_PIDFILE="$d/root.pid"
source "$here/stage-sampler.sh"
got=$(sample_stage)
old=$(ps -eo args | grep -q "[c]argo build --profile dist" && echo build-dist || echo other)
fail=""
[ "$got" = build-release ] || fail="tree sampler says '$got', want build-release (the dist build is a bystander)"
[ "$old" = build-dist ] || fail="$fail; premise: the old host-wide grep did not see the bystander ($old)"
[ -z "$fail" ] && echo PASS || { echo "FAIL: $fail"; exit 1; }
