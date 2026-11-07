#!/usr/bin/env bash
# Regression: a crashing tip_crates.py (or a failed git diff) left the crate list empty, the runner tested nothing and wrote the tip to
# tip-green.sha. A tip whose crate list could not be made is red. Stubs only: no cargo build, no live runner.
here=$(cd "$(dirname "$0")" && pwd)
d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
mkdir -p "$d/bin" "$d/tools" "$d/repo/crates/a" "$d/repo/tools"
cp "$here/red-tip.sh" "$here/tip_crates.py" "$d/tools/"
cat > "$d/bin/cargo" <<EOF
#!/bin/sh
case "\$1" in
  metadata) echo '{"workspace_root":"$d/repo","packages":[{"name":"a","manifest_path":"$d/repo/crates/a/Cargo.toml","dependencies":[]}]}' ;;
  test) echo "cargo test \$*" >> "$d/ran"; echo 'test x ... ok' ;;
esac
EOF
chmod +x "$d/bin/cargo"; export PATH="$d/bin:$PATH"
git -C "$d/repo" init -q && git -C "$d/repo" -c user.name=t -c user.email=t@t commit -q --allow-empty -m 0
echo 1 > "$d/repo/crates/a/lib.rs"; echo 1 > "$d/repo/tools/x"
git -C "$d/repo" add . && git -C "$d/repo" -c user.name=t -c user.email=t@t commit -q -m 1
source "$d/tools/red-tip.sh"
cd "$d/repo"
# control: the working script selects the changed package and the verdict runs its tests
[ "$(tip_crates HEAD~1 HEAD)" = "-p a" ] || { echo "FAIL: control: tip_crates gave '$(tip_crates HEAD~1 HEAD)'"; exit 1; }
tip_verdict "$d/repo" "$d/t" /nonexistent HEAD~1 HEAD >/dev/null || { echo "FAIL: control: a good tip was red"; exit 1; }
grep -q -- '-p a' "$d/ran" || { echo "FAIL: control: the tests of -p a did not run"; exit 1; }
# only tooling changed: nothing to test, green (the designed empty list)
git -c user.name=t -c user.email=t@t commit -q --allow-empty -m 2; echo 2 > tools/x; git add tools/x; git -c user.name=t -c user.email=t@t commit -q -m 3
rm -f "$d/ran"
tip_verdict "$d/repo" "$d/t" /nonexistent HEAD~1 HEAD >/dev/null || { echo "FAIL: a tooling-only tip was red"; exit 1; }
[ ! -e "$d/ran" ] || { echo "FAIL: a tooling-only tip ran tests"; exit 1; }
# the scanners: a stale generated page and a tracked .md naming ~/scratch each stop the runner, named (they were refused as batches, never blamed on the tip)
mkdir -p "$d/repo/tools/docs/docsbuild"; printf 'print("generated docs are out of date; run it")\nprint("  docs/x.md")\n' > "$d/repo/tools/docs/docsbuild/build.py"
stop=$(tip_verdict "$d/repo" "$d/t" /nonexistent HEAD~1 HEAD) && { echo "FAIL: a tip with a stale page was green"; exit 1; }
echo "$stop" | grep -q 'docs_in_sync.*docs/x.md' || { echo "FAIL: stale page not named: $stop"; exit 1; }
printf 'pass\n' > "$d/repo/tools/docs/docsbuild/build.py"
printf 'see ~/scratch/handoff/x.md\n' > "$d/repo/n.md"; git -C "$d/repo" add n.md
stop=$(tip_verdict "$d/repo" "$d/t" /nonexistent HEAD~1 HEAD) && { echo "FAIL: a tip naming ~/scratch was green"; exit 1; }
echo "$stop" | grep -q 'no_local_paths.*n.md:1' || { echo "FAIL: path not named: $stop"; exit 1; }
git -C "$d/repo" rm -qf n.md
printf 'made by cla''ude\n' > "$d/repo/m.txt"; git -C "$d/repo" add m.txt
stop=$(tip_verdict "$d/repo" "$d/t" /nonexistent HEAD~1 HEAD) && { echo "FAIL: a tip with a file naming the assistant was green"; exit 1; }
echo "$stop" | grep -q 'names the assistant.*m.txt' || { echo "FAIL: file not named: $stop"; exit 1; }
git -C "$d/repo" rm -qf m.txt
tip_verdict "$d/repo" "$d/t" /nonexistent HEAD~1 HEAD >/dev/null || { echo "FAIL: a clean tip was red"; exit 1; }
# the crash: tip_crates.py dies
printf 'raise SystemExit("boom")\n' > "$d/tools/tip_crates.py"
tip_crates HEAD~2 HEAD >/dev/null 2>&1 && { echo "FAIL: tip_crates returned 0 although tip_crates.py crashed"; exit 1; }
rm -f "$d/ran"
stop=$(tip_verdict "$d/repo" "$d/t" /nonexistent HEAD~2 HEAD) && { echo "FAIL: a tip whose crate list crashed was green: $stop"; exit 1; }
echo "$stop" | grep -q '^STOP: release/next red: .*tip_crates' || { echo "FAIL: stop line: $stop"; exit 1; }
[ ! -e "$d/ran" ] || { echo "FAIL: tests ran with no crate list"; exit 1; }
echo PASS
