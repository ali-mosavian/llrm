# dist_extra_bins WORKTREE: the C-frontend binaries a commit builds beyond the four every dist has (llrm-cc, llrm-llc), one name per word; older commits have none, and a `--bin` that does not exist fails the build.
dist_extra_bins() {
  local w=$1 b out=
  for b in llrm-cc llrm-llc; do
    grep -qs "^name = \"$b\"" "$w/Cargo.toml" "$w"/crates/driver/*/Cargo.toml "$w"/crates/driver/*/*/Cargo.toml && out="$out $b"
  done
  echo $out
}
