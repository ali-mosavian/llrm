#!/usr/bin/env bash
# Regression: landing fixes waited 15 min after their last push like any PR, though their owners do not push during a batch; an urgent batch admits a head 5 minutes old.
here=$(cd "$(dirname "$0")" && pwd); source "$here/head-age.sh"
[ "$(batch_head_age 0)" = 900 ] || { echo "FAIL: a normal batch is not 15 min: $(batch_head_age 0)"; exit 1; }
[ "$(batch_head_age 2)" = 300 ] || { echo "FAIL: an urgent batch is not 5 min: $(batch_head_age 2)"; exit 1; }
[ "$(HEAD_MIN_AGE=60 batch_head_age 0)" = 60 ] && [ "$(URGENT_HEAD_MIN_AGE=120 batch_head_age 1)" = 120 ] || { echo "FAIL: the environment does not override"; exit 1; }
grep -q 'HEAD_MIN_AGE=$(batch_head_age' "$here/queue-runner.sh" || { echo "FAIL: queue-runner does not age by batch kind"; exit 1; }
echo PASS urgent-age
