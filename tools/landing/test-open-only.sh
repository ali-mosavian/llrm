#!/usr/bin/env bash
# Regression: the runner passed MERGED PRs (#2828, #2860 landed by hand mid-batch) to merge-batch, which would refuse or re-squash them.
d=$(mktemp -d); printf '#!/bin/sh\ncase "$3" in 2828|2860) echo MERGED;; *) echo OPEN;; esac\n' > $d/gh; chmod +x $d/gh
PATH=$d:$PATH; source "$(dirname "$0")/open-only.sh"
out=$(open_only 2828 2860 2721 2857 2>/dev/null | tr '\n' ' ')
[ "$out" = "2721 2857 " ] && echo PASS || { echo "FAIL: got '$out'"; exit 1; }

# Regression: the batch was popped from the queue file before open_only ran, so each empty-batch loop deleted its candidates from priority.txt (~110 entries gone).
open_only() { for m; do [ "$m" != 7 ] && echo "$m"; done; }
q=$(mktemp); printf '5\n7\n9\n' > $q
got=$(pop_open $q 5 7); [ "$got" = 5 ] && [ "$(tr '\n' ' ' < $q)" = "7 9 " ] || { echo "FAIL: pop_open '$got' left '$(tr '\n' ' ' < $q)'"; exit 1; }
got=$(pop_open $q); [ -z "$got" ] && [ "$(tr '\n' ' ' < $q)" = "7 9 " ] || { echo "FAIL: an empty batch changed the queue"; exit 1; }
echo PASS pop_open

# Regression: #5391 was drafted at 18:29 to keep it out of a batch; the runner kept it as a candidate (open, in the queue built earlier) and `gh pr ready` un-drafted it at 19:10:47, 12 s before sqb-5300 started.
d=$(mktemp -d); printf '#!/bin/sh\ncase "$3" in 5391) echo "OPEN true";; *) echo "OPEN false";; esac\n' > $d/gh; chmod +x $d/gh
PATH=$d:$PATH; source "$(dirname "$0")/open-only.sh"
out=$(open_only 5300 5391 5402 2>/dev/null | tr '\n' ' ')
[ "$out" = "5300 5402 " ] || { echo "FAIL: a draft stayed in the batch: '$out'"; exit 1; }
grep -q 'gh pr ready' "$(dirname "$0")/queue-runner.sh" && { echo "FAIL: queue-runner.sh readies a draft"; exit 1; }
echo PASS draft
# Regression: an urgent batch with nothing to land became the literal `feat/parity`; `gh pr ready feat/parity` resolved the branch to main PR #5663 and marked it ready for review (02:38). A name that is not a PR number never passes open_only.
d=$(mktemp -d); printf '#!/bin/sh\necho OPEN\n' > $d/gh; chmod +x $d/gh
PATH=$d:$PATH; source "$(dirname "$0")/open-only.sh"
out=$(open_only feat/parity 5 2>/dev/null | tr '\n' ' ')
[ "$out" = "5 " ] && echo PASS numeric || { echo "FAIL: a branch name passed open_only: '$out'"; exit 1; }
