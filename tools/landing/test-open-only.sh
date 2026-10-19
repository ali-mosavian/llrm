#!/usr/bin/env bash
# Regression: the runner passed MERGED PRs (#2828, #2860 landed by hand mid-batch) to merge-batch, which would refuse or re-squash them.
d=$(mktemp -d); printf '#!/bin/sh\ncase "$3" in 2828|2860) echo MERGED;; *) echo OPEN;; esac\n' > $d/gh; chmod +x $d/gh
PATH=$d:$PATH; source "$(dirname "$0")/open-only.sh"
out=$(open_only 2828 2860 2721 2857 2>/dev/null | tr '\n' ' ')
[ "$out" = "2721 2857 " ] && echo PASS || { echo "FAIL: got '$out'"; exit 1; }
