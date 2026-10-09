#!/usr/bin/env bash
# Format the tree: nightly rustfmt, then tools/rfmt-post (method chains, long matches!). Plain `cargo fmt` is not enough.
#
#   tools/fmt.sh                rewrite every file rustfmt formats
#   tools/fmt.sh --check        change nothing; list the files that would change; exit 1 if any
#   tools/fmt.sh --stdin FILE   FILE's text on stdin, formatted on stdout (editors); a failure writes nothing
#
# Exit status: 0 clean, 1 --check found files to format, 2 or more anything else (a missing toolchain, a failed build, a
# crash). A gate that tolerates 1 must not tolerate the rest.
#
# Per file: rustfmt, break long matches!, rustfmt again (it lays out around them), split chains. Running it twice
# changes nothing.
set -euo pipefail

verdict=
trap 'rc=$?; [ "$rc" = 1 ] && [ "$verdict" != changed ] && exit 2' EXIT
fail() { echo "fmt: $1" >&2; exit 2; }

here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
config=$root/rustfmt.toml

# rustfmt's `ignore` applies to files it is handed; stdin and the per-file workers have to honour it themselves.
ignored() {
    local rel=${1#"$root"/} one
    for one in $(sed -n 's/^ignore *= *\[\(.*\)\]/\1/p' "$config" | tr -d '", '); do
        [ "$rel" = "$one" ] && return 0
    done
    return 1
}

# The edition cargo would pass for FILE: its nearest Cargo.toml's own, else the workspace's.
edition_of() {
    local dir
    dir=$(dirname "$1")
    while [ "$dir" != / ] && [ "$dir" != . ]; do
        if [ -f "$dir/Cargo.toml" ]; then
            sed -n 's/^edition *= *"\([0-9]*\)".*/\1/p' "$dir/Cargo.toml" | head -n 1 | grep . && return
            break
        fi
        dir=$(dirname "$dir")
    done
    echo 2024
}

# stdin -> stdout, as the path FILE
pipeline() {
    local rustfmt=(rustfmt +nightly --edition "$(edition_of "$1")" --config-path "$config")
    "${rustfmt[@]}" | "$RFMT_POST" --matches --stdin | "${rustfmt[@]}" | "$RFMT_POST" --stdin
}

# Format FILE; print its path when that changed it, and write it when RFMT_WRITE=1.
one() {
    local out
    ignored "$1" && return 0
    out=$(mktemp)
    trap 'rm -f "$out"' RETURN
    if ! pipeline "$1" < "$1" > "$out"; then
        echo "fmt: $1: failed" >&2
        return 1
    fi
    cmp -s "$1" "$out" && return 0
    echo "$1"
    [ "${RFMT_WRITE:-0}" = 1 ] && cat "$out" > "$1"
    return 0
}

# Its own target directory, beside the workspace's: the tool shares no lock or dependency with the build it formats.
build_tool() {
    local dir
    dir=$(python3 "$here/llrmbin.py" target)/rfmt-post
    cargo build --release -q --manifest-path "$here/rfmt-post/Cargo.toml" --target-dir "$dir" >&2 || return 2
    echo "$dir/release/rfmt-post"
}

# The files cargo fmt formats, one `cargo fmt` with the given arguments. --check exits 1 on a difference, so only 2 or more is a
# failure there; an empty list is one too (cargo +nightly not found, a crash before the first file).
listed() {
    local out rc=0
    out=$(cd "$root" && cargo +nightly fmt "$@" -- --check -v) || rc=$?
    [ "$rc" -le 1 ] || fail "cargo fmt $* failed ($rc)"
    printf '%s\n' "$out" | LC_ALL=C sed -n 's/^Formatting //p'
}

# The files `cargo fmt --all` formats, and the tool's own sources (outside the workspace); each once.
files() {
    local all tool
    all=$(listed --all) && tool=$(listed --manifest-path tools/rfmt-post/Cargo.toml) || exit 2
    [ -n "$all" ] || fail "cargo fmt listed no files"
    printf '%s\n%s\n' "$all" "$tool" | sort -u
}

mode=${1:-rewrite}
case $mode in
--one)
    : "${RFMT_POST:?}"
    one "$2"
    ;;
--stdin)
    [ $# = 2 ] || { echo "usage: tools/fmt.sh --stdin FILE" >&2; exit 2; }
    RFMT_POST=$(build_tool) || fail "building tools/rfmt-post failed"
    if ignored "$2"; then cat; else pipeline "$2"; fi
    ;;
rewrite | --check)
    [ $# -le 1 ] || { echo "usage: tools/fmt.sh [--check | --stdin FILE]" >&2; exit 2; }
    export RFMT_POST RFMT_WRITE=$([ "$mode" = rewrite ] && echo 1 || echo 0)
    cargo +nightly fmt --version > /dev/null 2>&1 || fail "no nightly rustfmt (rustup toolchain install nightly -c rustfmt)"
    RFMT_POST=$(build_tool) || fail "building tools/rfmt-post failed"
    jobs=${FMT_JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu)}
    list=$(files) || exit 2
    changed=$(printf '%s\n' "$list" | xargs -P "$jobs" -n 1 "$0" --one | sort)
    [ -z "$changed" ] && exit 0
    count=$(echo "$changed" | wc -l | tr -d ' ')
    if [ "$mode" = rewrite ]; then
        echo "fmt: formatted $count files" >&2
    else
        echo "${changed//"$root"\//}"
        echo "fmt: $count files would change; run tools/fmt.sh" >&2
        verdict=changed
        exit 1
    fi
    ;;
*)
    echo "usage: tools/fmt.sh [--check | --stdin FILE]" >&2
    exit 2
    ;;
esac
