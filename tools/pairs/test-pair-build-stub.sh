#!/usr/bin/env bash
# pair_build with a stubbed cargo: the cargo line and the ok record. A real dist is a 20+ minute build under a shared lock.
here=$(cd "$(dirname "$0")" && pwd); r=$(mktemp -d); repo=$r/repo; S=$r/scratch; mkdir -p $repo $S
stub=$r/cargo; cat > $stub <<'EOS'
#!/bin/bash
# records its arguments; "builds" every --bin into $CARGO_TARGET_DIR/dist, each .d naming the worktree
echo "$@" > "$STUB_LOG"
mkdir -p "$CARGO_TARGET_DIR/dist"
while [ $# -gt 0 ]; do if [ "$1" = --bin ]; then echo "bin $2" > "$CARGO_TARGET_DIR/dist/$2"; echo "$CARGO_TARGET_DIR/dist/$2: $PWD/src/main.rs" > "$CARGO_TARGET_DIR/dist/$2.d"; fi; shift; done
EOS
chmod +x $stub
cd $repo && git init -q -b main && git config user.email t@t && git config user.name t
printf '[[bin]]\nname = "llrm-llc"\n' > Cargo.toml; git add -A && git commit -qm new && new=$(git rev-parse HEAD)
STUB_LOG=$r/log PAIR_CARGO=$stub bash $here/pair_build.sh $S $new >/dev/null 2>$r/err || { echo "FAIL: pair_build: $(cat $r/err)"; exit 1; }
want="build --profile dist -q --bin llrm-qb --bin llrm-nib --bin nibfront --bin llrm-cc --bin llrm-llc"
[ "$(cat $r/log)" = "$want" ] || { echo "FAIL: cargo line: '$(cat $r/log)'"; exit 1; }
names() { grep -E '^[0-9a-f]{32}  ' $1/ok | awk '{print $2}' | grep -v '\.d$' | tr '\n' ' '; }
[ "$(names $S/sbp-$new)" = "llrm-cc llrm-llc llrm-nib llrm-qb nibfront " ] || { echo "FAIL: ok record: '$(names $S/sbp-$new)'"; exit 1; }
grep -q -- "--bin nibfront --bin llrm-cc --bin llrm-llc" $S/sbp-$new/ok || { echo "FAIL: the ok does not record its build line"; exit 1; }
echo PASS pair-build-stub
