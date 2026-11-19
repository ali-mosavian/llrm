#!/bin/bash
# The built llrm-cc reads data files from the source tree it was built in; rerun.sh once deleted that tree, so every
# compile in the bench failed ("datalayout.toml: No such file"). The binary must still run after rerun.sh returns.
S=${1:-$(dirname "$0")/rerun.sh}; T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
R=$T/r; git init -q "$R"; mkdir -p "$R/src/bin"
printf '[package]\nname="f"\nversion="0.1.0"\nedition="2021"\n' > "$R/Cargo.toml"; echo data > "$R/data.txt"
for b in llrm-cc llrm-arm64-llc; do cat > "$R/src/bin/$b.rs" <<R2
fn main() { std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data.txt")).expect("data file missing"); println!("$1"); }
R2
done
echo '/target' > "$R/.gitignore"; git -C "$R" add -A; for v in a b; do sed -i '' "s/println!(\"[^\"]*\")/println!(\"$v\")/" "$R/src/bin/llrm-cc.rs"; git -C "$R" -c user.name=t -c user.email=t@t commit -q -am "$v" --allow-empty; done
H=$(git -C "$R" rev-parse HEAD); B=$(git -C "$R" rev-parse HEAD~1)
REPO=$R W=$T/w OUT=$T/o LLRM_PIN_CLANG=/bin/true PROGS=x bash "$S" "$H" "$B" O2 >/dev/null 2>&1
"$T/w/bin/${H:0:12}/llrm-cc" >/dev/null 2>&1 && echo PASS || { echo "FAIL: built llrm-cc does not run after rerun.sh"; exit 1; }
