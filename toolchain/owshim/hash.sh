#!/bin/sh
# What wccq is made from: the Open Watcom commit and every file of ours that
# goes into it. build.sh stamps its output with this; a test compares.
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
# The descriptions the front end is built from (build.rs): its sizes are the targets' own.
sum=$(cd "$HERE" && cat ow-commit build.sh cgshim.c cc-objects.txt patches/*.patch ../../runtime/c/*/c.toml ../../crates/target/llrm-x86-m*/src/machines/datalayout.toml | sha256sum | cut -c1-16)
echo "$(cat "$HERE/ow-commit")-$sum-${OWCPU:-i86}"
