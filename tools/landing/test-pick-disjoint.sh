#!/usr/bin/env bash
# Regression: a batch of 7 landed 0, 4 refused for conflicts with batch mates (shared lib.rs, facts_rewrite.rs, parity.rs). Two PRs that touch the same file never share a batch.
here=$(cd "$(dirname "$0")" && pwd); source "$here/pick-disjoint.sh"; d=$(mktemp -d)
cat > $d/gh <<'S'
#!/bin/sh
case "$3" in
 1) printf 'a.rs\nb.rs\ndocs/optimizations/passes/readme.md\n';;
 2) printf 'b.rs\nc.rs\n';;
 3) printf 'd.rs\ndocs/optimizations/passes/readme.md\n';;
 4) printf 'a.rs\n';;
 5) printf 'e.rs\n';;
esac
S
chmod +x $d/gh; PATH=$d:$PATH
out=$(pick_disjoint 8 1 2 3 4 5 | tr '\n' ' ')
[ "$out" = "1 3 5 " ] || { echo "FAIL: got '$out', want '1 3 5 '"; exit 1; }
out=$(pick_disjoint 2 1 2 3 4 5 | tr '\n' ' ')
[ "$out" = "1 3 " ] || { echo "FAIL: max not honoured: '$out'"; exit 1; }
echo PASS
