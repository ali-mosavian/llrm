#!/usr/bin/env bash
# Regression: #4713 was admitted two minutes after its owner pushed a hand merge with a lost brace and refused the batch; a head younger than 15 minutes waits.
# A closed PR is dropped (open_only), and so is one whose date cannot be read.
here=$(cd "$(dirname "$0")" && pwd); source "$here/head-age.sh"; source "$here/open-only.sh"; d=$(mktemp -d)
old=$(date -u -d '30 minutes ago' +%FT%TZ); new=$(date -u -d '2 minutes ago' +%FT%TZ)
cat > $d/gh <<S
#!/bin/sh
case "\$3" in
 1) echo $old;;
 2) echo $new;;
 3) echo $old;;
 4) exit 1;;
esac
S
chmod +x $d/gh; PATH=$d:$PATH
out=$(aged 1 2 3 4 2>/dev/null | tr '\n' ' ')
[ "$out" = "1 3 " ] || { echo "FAIL: got '$out', want '1 3 ' (2 is 2 minutes old, 4 has no date)"; exit 1; }
HEAD_MIN_AGE=60 out=$(aged 2 | tr '\n' ' ')
[ "$out" = "2 " ] || { echo "FAIL: HEAD_MIN_AGE=60 should admit a 2-minute-old head: '$out'"; exit 1; }
unset HEAD_MIN_AGE
# a file the coordinator edits sets the age with no restart: 60 s admits the 2-minute head, junk falls back to the default
echo 60 > $d/head-age.txt; out=$(LAND_TMP=$d aged 2 | tr '\n' ' ')
[ "$out" = "2 " ] || { echo "FAIL: head-age.txt=60 should admit a 2-minute-old head: '$out'"; exit 1; }
echo 3600 > $d/head-age.txt; out=$(LAND_TMP=$d aged 1 2>/dev/null | tr '\n' ' ')
[ -z "$out" ] || { echo "FAIL: head-age.txt=3600 should hold a 30-minute-old head: '$out'"; exit 1; }
echo soon > $d/head-age.txt; out=$(LAND_TMP=$d aged 1 2 2>/dev/null | tr '\n' ' ')
[ "$out" = "1 " ] || { echo "FAIL: a junk head-age.txt should fall back to 900 s: '$out'"; exit 1; }
cat > $d/gh <<'S'
#!/bin/sh
case "$3" in 7) echo OPEN;; 8) echo CLOSED;; esac
S
out=$(open_only 7 8 2>/dev/null | tr '\n' ' ')
[ "$out" = "7 " ] || { echo "FAIL: a closed PR was kept: '$out'"; exit 1; }
echo PASS
