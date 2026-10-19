#!/usr/bin/env bash
# Regression: a batch of 8 where three PRs conflict in lib.rs refused all of them, though each merges alone. Only the "does not merge with the others" refusal retries singly.
here=$(cd "$(dirname "$0")" && pwd); source "$here/retry-singly.sh"; d=$(mktemp -d)
printf '%s\n' '15:41:07 #2845: does not merge with the others: Auto-merging crates/ir/x.rs' '15:41:07 #2836: docs/jumps.md: the squash deletes 30 line(s)' > $d/log
retry_singly $d/log 2845 || { echo "FAIL: a conflict with batch mates did not retry"; exit 1; }
retry_singly $d/log 2836 && { echo "FAIL: a deletion-rule refusal retried"; exit 1; }
retry_singly $d/log 9999 && { echo "FAIL: absent PR retried"; exit 1; }
echo PASS
