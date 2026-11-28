#!/usr/bin/env bash
# Regression: a dist had llrm-cc but not llrm-llc, the frontends C2 switches to; a commit is asked only for the binaries it has; and a dist
# without a backend's llc (llrm-m68k-llc) measured that target with whatever llc was beside it.
. "$(dirname "$0")/dist-bins.sh"
d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
mkdir -p $d/new/crates/driver/llrm-cc $d/new/crates/target/llrm-m68k $d/old
printf '[[bin]]\nname = "llrm-llc"\npath = "x.rs"\n' > $d/new/Cargo.toml; printf '[package]\nname = "llrm-cc"\n[[bin]]\nname = "llrm-cc"\n' > $d/new/crates/driver/llrm-cc/Cargo.toml
printf '[package]\nname = "llrm-m68k"\n[[bin]]\nname = "llrm-m68k-llc"\n' > $d/new/crates/target/llrm-m68k/Cargo.toml
printf '[[bin]]\nname = "llrm-cc"\n' > $d/old/Cargo.toml
[ "$(dist_extra_bins $d/new)" = "llrm-cc llrm-llc llrm-m68k-llc" ] || { echo "FAIL: new tree: '$(dist_extra_bins $d/new)'"; exit 1; }
[ "$(dist_extra_bins $d/old)" = "llrm-cc" ] || { echo "FAIL: old tree asked for '$(dist_extra_bins $d/old)'"; exit 1; }
echo PASS dist-bins
