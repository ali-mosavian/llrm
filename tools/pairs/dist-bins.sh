# dist_extra_bins WORKTREE: the binaries a commit builds beyond the four every dist has: llrm-cc, llrm-llc and every backend's llc (llrm-*-llc),
# one name per word; older commits have fewer, and a `--bin` that does not exist fails the build. A dist without a backend's llc measured that
# target with whatever llc was beside it, and its `ok` record could not vouch for one (tools/stamp.py same_build).
dist_extra_bins() {
  local w=$1 out=
  grep -qs '^name = "llrm-cc"' "$w/Cargo.toml" "$w"/crates/driver/*/Cargo.toml "$w"/crates/driver/*/*/Cargo.toml && out="$out llrm-cc"
  grep -qs '^name = "llrm-llc"' "$w/Cargo.toml" "$w"/crates/driver/*/Cargo.toml "$w"/crates/driver/*/*/Cargo.toml && out="$out llrm-llc"
  out="$out $(cat "$w"/crates/target/*/Cargo.toml 2>/dev/null | sed -n 's/^name = "\(llrm-[a-z0-9]*-llc\)"$/\1/p' | sort -u | tr '\n' ' ')"
  echo $out
}

# dist_features WORKTREE: the --features the commit'"'"'s four dist bins require (their required-features, read from its Cargo.toml: llrm-cc needs c-legacy), empty for a commit that has none.
dist_features() {
  python3 - "$1/Cargo.toml" <<'EOP'
import sys, tomllib
try:
    bins = tomllib.load(open(sys.argv[1], "rb")).get("bin", [])
except OSError:
    bins = []
want = {f for b in bins if b.get("name") in ("llrm-cc", "llrm-qb", "llrm-nib", "nibfront") for f in b.get("required-features", [])}
print(",".join(sorted(want)))
EOP
}
