#!/bin/bash
# A/B guard: ab_guard.sh <repo> <base> <head> <base-bin> <head-bin> [counts-file...]
# Refuses (exit 2, one line) unless base is the newest upstream commit head contains
# (upstream: $AB_UPSTREAM, default origin/release/next), the binaries differ, and every counts file is non-empty.
repo=$1 base=$2 head=$3 bbin=$4 hbin=$5; shift 5
no() { echo "ab_guard: $*" >&2; exit 2; }
up=${AB_UPSTREAM:-origin/release/next}
b=$(git -C "$repo" rev-parse --verify -q "$base^{commit}") || no "base $base is not a commit"
h=$(git -C "$repo" rev-parse --verify -q "$head^{commit}") || no "head $head is not a commit"
want=$(git -C "$repo" merge-base "$h" "$up") || no "no merge-base of head and $up"
[ "$b" = "$want" ] || no "stale base: base ${b:0:10} is not the newest $up commit head contains (${want:0:10})"
[ "$(md5sum < "$bbin")" != "$(md5sum < "$hbin")" ] || no "base and head binaries are identical"
for c in "$@"; do [ -s "$c" ] || no "counts file $c is empty or missing"; done
