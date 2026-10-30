#!/usr/bin/env bash
# Regression: the runner read its scripts, helpers and env once, so a merged shell fix or a HEAD_MIN_AGE change waited for the user's restart (the alone-filter and head-age fixes of 2026-10-27).
here=$(cd "$(dirname "$0")" && pwd); source "$here/runner-reload.sh"
d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
echo 'X=1' > $d/runner.env; unset X; runner_env_load $d/runner.env; [ "$X" = 1 ] || { echo "FAIL: runner.env not loaded"; exit 1; }
echo 'X=2' > $d/runner.env; runner_env_load $d/runner.env; [ "$X" = 2 ] || { echo "FAIL: a changed runner.env was not picked up"; exit 1; }
runner_env_load $d/none || { echo "FAIL: a missing runner.env failed the runner"; exit 1; }
mkdir $d/s; echo a > $d/s/queue-runner.sh; echo a > $d/s/head-age.sh; echo a > $d/s/test-head-age.sh
before=$(runner_scripts_sha $d/s); echo b > $d/s/test-head-age.sh; [ "$(runner_scripts_sha $d/s)" = "$before" ] || { echo "FAIL: a test file reloaded the runner"; exit 1; }
echo b > $d/s/head-age.sh; [ "$(runner_scripts_sha $d/s)" != "$before" ] || { echo "FAIL: a changed helper did not change the runner's sha"; exit 1; }
awk '/^while true/{w=1} w&&/runner_env_load/{e=1} w&&/runner_scripts_sha/{s=1} END{exit !(e&&s)}' "$here/queue-runner.sh" || { echo "FAIL: queue-runner does not reload inside its loop"; exit 1; }
echo PASS runner-reload
# Regression: the runner called runner_env_load and compared $SELF_SHA but never sourced runner-reload.sh or set SELF_SHA: runner.env was ignored ('command not found' at 10:07), so LAND_SKIP_ALONE=1 ran the alone check anyway (10:13).
r=$here/queue-runner.sh
grep -q 'source "$HERE/runner-reload.sh"' $r || { echo "FAIL: queue-runner.sh does not source runner-reload.sh"; exit 1; }
grep -q '^SELF_SHA=$(runner_scripts_sha "$HERE")' $r || { echo "FAIL: queue-runner.sh never sets SELF_SHA"; exit 1; }
d=$(mktemp -d); echo "LAND_SKIP_ALONE=1" > $d/runner.env; ( runner_env_load $d/runner.env; [ "$LAND_SKIP_ALONE" = 1 ] && [ -n "$(env | grep ^LAND_SKIP_ALONE=1)" ] ) || { echo "FAIL: runner_env_load does not export"; exit 1; }
echo PASS runner-source
