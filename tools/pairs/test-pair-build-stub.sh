#!/usr/bin/env bash
# pair_build with a stubbed cargo: the cargo line and the ok record for an old commit (four bins) and a current one (six). A real dist is a 20+ minute build under a shared lock.
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
printf '[[bin]]\nname = "llrm-c"\n' > Cargo.toml; git add -A && git commit -qm old && old=$(git rev-parse HEAD)
mkdir -p crates/driver/llrm-cc && printf '[package]\nname = "llrm-cc"\n[[bin]]\nname = "llrm-cc"\n' > crates/driver/llrm-cc/Cargo.toml && printf '[[bin]]\nname = "llrm-c"\n[[bin]]\nname = "llrm-llc"\n' > Cargo.toml
mkdir -p tools/pairs && cp $here/dist-bins.sh tools/pairs/; git add -A && git commit -qm new && new=$(git rev-parse HEAD)
for sha in $old $new; do
  STUB_LOG=$r/log-$sha PAIR_CARGO=$stub bash $here/pair_build.sh $S $sha >/dev/null 2>$r/err-$sha || { echo "FAIL: pair_build $sha: $(cat $r/err-$sha)"; exit 1; }
done
want_old="build --profile dist -q --bin llrm-c --bin llrm-qb --bin llrm-nib --bin nibfront"
[ "$(cat $r/log-$old)" = "$want_old" ] || { echo "FAIL: old commit cargo line: '$(cat $r/log-$old)'"; exit 1; }
[ "$(cat $r/log-$new)" = "$want_old --bin llrm-cc --bin llrm-llc" ] || { echo "FAIL: new commit cargo line: '$(cat $r/log-$new)'"; exit 1; }
names() { grep -E '^[0-9a-f]{32}  ' $1/ok | awk '{print $2}' | grep -v '\.d$' | tr '\n' ' '; }
[ "$(names $S/sbp-$old)" = "llrm-c llrm-nib llrm-qb nibfront " ] || { echo "FAIL: old ok record: '$(names $S/sbp-$old)'"; exit 1; }
[ "$(names $S/sbp-$new)" = "llrm-c llrm-cc llrm-llc llrm-nib llrm-qb nibfront " ] || { echo "FAIL: new ok record: '$(names $S/sbp-$new)'"; exit 1; }
grep -q -- "--bin nibfront --bin llrm-cc --bin llrm-llc" $S/sbp-$new/ok || { echo "FAIL: the new ok does not record its build line"; exit 1; }
echo PASS pair-build-stub; echo "old: $(cat $r/log-$old)"; echo "new: $(cat $r/log-$new)"; echo "old ok: $(names $S/sbp-$old)"; echo "new ok: $(names $S/sbp-$new)"
