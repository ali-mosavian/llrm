#!/usr/bin/env bash
# Regression: a dist had llrm-c but neither llrm-cc nor llrm-llc, the frontends C2 switches to; and a commit without them must not be asked to build them.
. "$(dirname "$0")/dist-bins.sh"
d=$(mktemp -d); mkdir -p $d/new/crates/driver/llrm-cc $d/old
printf '[[bin]]\nname = "llrm-llc"\npath = "x.rs"\n' > $d/new/Cargo.toml; printf '[package]\nname = "llrm-cc"\n[[bin]]\nname = "llrm-cc"\n' > $d/new/crates/driver/llrm-cc/Cargo.toml
printf '[[bin]]\nname = "llrm-c"\n' > $d/old/Cargo.toml
[ "$(dist_extra_bins $d/new)" = "llrm-cc llrm-llc" ] || { echo "FAIL: new tree: '$(dist_extra_bins $d/new)'"; exit 1; }
[ -z "$(dist_extra_bins $d/old)" ] || { echo "FAIL: old tree asked for '$(dist_extra_bins $d/old)'"; exit 1; }
echo PASS dist-bins
