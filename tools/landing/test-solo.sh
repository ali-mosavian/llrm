#!/usr/bin/env bash
# Regression: #4993 (300+ generated docs files) conflicted after every parity move because it rode in a batch of 16; a listed PR is a batch of its own.
source "$(dirname "$0")/solo.sh"
f=$(mktemp); printf '# comment 55\n4993 # docs resync\n' > $f
[ "$(solo_listed $f 4993)" = 4993 ] || { echo "FAIL: a listed PR is not solo"; exit 1; }
[ -z "$(solo_listed $f 55)" ] && [ -z "$(solo_listed $f 499)" ] && [ -z "$(solo_listed $f 4994)" ] || { echo "FAIL: a comment or a prefix made a PR solo"; exit 1; }
[ -z "$(solo_listed /nonexistent 4993)" ] || { echo "FAIL: a missing file made a PR solo"; exit 1; }
echo PASS solo
