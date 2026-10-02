#!/usr/bin/env bash
# Every stage dump -- MIR after each pass, LIR, emitted assembly -- for the OMF
# corpus and the Nib examples, so a representation change is judged by
# `diff -r` against the tree from before it. Refusals are recorded, not fatal.
#
# usage: tools/baseline.sh OUT
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:?usage: tools/baseline.sh OUT}
bin=$root/target/release
build=(cargo build --release --quiet --manifest-path "$root/Cargo.toml" --bin llrm-omf --bin llrm-nib)
"${build[@]}" 2>/dev/null || "${build[@]}"

# record DIR COMMAND...: the dumps in DIR, and its exit status and stderr beside them
record() {
    local dir=$1; shift
    mkdir -p "$dir"
    local status=0
    "$@" >/dev/null 2>"$dir/stderr" || status=$?
    echo "$status" >"$dir/status"
}

for object in "$root"/tests/fixtures/omf/*.obj; do
    name=$(basename "$object" .obj)
    record "$out/omf/$name" "$bin/llrm-omf" "$object" --dump "$out/omf/$name" --quiet
done
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
for source in "$root"/examples/*.nib; do
    name=$(basename "$source" .nib)
    record "$out/nib/$name" "$bin/llrm-nib" "$source" -o "$scratch/$name.obj" --dump "$out/nib/$name"
    [ -f "$scratch/$name.obj" ] && sha256sum <"$scratch/$name.obj" | cut -d' ' -f1 >"$out/nib/$name/object.sha256"
done
