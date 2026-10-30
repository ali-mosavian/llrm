#!/usr/bin/env bash
# Regression: alone and assemble ran parity_squash.py from a worktree that the previous cycle's clean_checkout left behind, so every tooling merge (#5120, #5123) took effect one cycle late.
here=$(cd "$(dirname "$0")" && pwd); source "$here/tooling-current.sh"
d=$(mktemp -d); trap 'rm -rf "$d"' EXIT; cd "$d"
git init -q --bare o.git; git clone -q o.git w 2>/dev/null; cd w; git config user.email t@t; git config user.name t
mkdir -p tools/landing; git checkout -q -b release/next; echo 1 > tools/f; git add .; git commit -qm one; old=$(git rev-parse HEAD); git push -q origin release/next
git checkout -q --detach; echo 2 > tools/f; git commit -qam two; git push -q origin HEAD:release/next; new=$(git rev-parse HEAD); git checkout -q --detach "$old"
tooling_current "$d/w" >/dev/null 2>&1
[ "$(git -C "$d/w" rev-parse HEAD)" = "$new" ] || { echo "FAIL: the tooling worktree stayed one merge behind origin/release/next"; exit 1; }
grep -n 'tooling_current\|alone_filter "' "$here/queue-runner.sh" | awk -F: '/tooling_current/{t=$1} /alone_filter "/{a=$1} END{exit !(t && a && t<a)}' || { echo "FAIL: queue-runner does not refresh the tooling before alone_filter"; exit 1; }
echo PASS tooling-current
