# dist_extra_bins WORKTREE: the binaries a commit builds beyond the four every dist has (llrm-cc, llrm-llc, llrm-arm64-llc), one name per word; older commits have none, and a `--bin` that does not exist fails the build.
dist_extra_bins() {
  local w=$1 b out=
  for b in llrm-cc llrm-llc llrm-arm64-llc; do
    grep -qs "^name = \"$b\"" "$w/Cargo.toml" "$w"/crates/driver/*/Cargo.toml "$w"/crates/driver/*/*/Cargo.toml "$w"/crates/target/*/Cargo.toml && out="$out $b"
  done
  echo $out
}

# dist_features WORKTREE: the --features the commit'"'"'s four dist bins require (their required-features, read from its Cargo.toml: llrm-c needs c-legacy), empty for a commit that has none.
dist_features() {
  python3 - "$1/Cargo.toml" <<'EOP'
import sys, tomllib
try:
    bins = tomllib.load(open(sys.argv[1], "rb")).get("bin", [])
except OSError:
    bins = []
want = {f for b in bins if b.get("name") in ("llrm-c", "llrm-qb", "llrm-nib", "nibfront") for f in b.get("required-features", [])}
print(",".join(sorted(want)))
EOP
}
