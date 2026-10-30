#!/usr/bin/env bash
# Unpacks Ubuntu's arm64 cross libc and libgcc into ~/.cache/llrm-arm64-sysroot (ARM64_SYSROOT overrides) so that
# llrm-cc (the arm64-glibc row) builds static arm64 Linux programs that qemu-aarch64 runs. No root: apt-get download and dpkg -x.
# Prints the sysroot's root. Needs ld.lld and qemu-aarch64 on the path.
set -euo pipefail

# The newest libgcc-N-dev-arm64-cross among the package names on stdin; the runner's Ubuntu may not carry 15.
newest_libgcc() {
    grep -E '^libgcc-[0-9]+-dev-arm64-cross$' | sort -t- -k2,2n | tail -n 1
}
if [ "${1:-}" = "--newest-libgcc" ]; then
    newest_libgcc
    exit 0
fi

prefix=${ARM64_SYSROOT:-$HOME/.cache/llrm-arm64-sysroot}
done_marker=$prefix/root/.complete
if [ -f "$done_marker" ]; then
    echo "$prefix/root"
    exit 0
fi
mkdir -p "$prefix"
# One builder at a time: concurrent extractions into one tree unlinked each other's files (dpkg-deb "Cannot unlink"), and a
# half-extracted root passed the old directory test and segfaulted a program. The root appears by rename, marker last.
exec 9>"$prefix/.lock"
flock 9
if [ -f "$done_marker" ]; then
    echo "$prefix/root"
    exit 0
fi
libgcc=$(apt-cache pkgnames libgcc- | newest_libgcc)
[ -n "$libgcc" ] || { echo "no libgcc-N-dev-arm64-cross package in apt" >&2; exit 1; }
mkdir -p "$prefix/debs"
(cd "$prefix/debs" && apt-get download libc6-dev-arm64-cross libc6-arm64-cross linux-libc-dev-arm64-cross \
    libgcc-s1-arm64-cross "$libgcc" >&2)
rm -rf "$prefix/root" "$prefix/root.new"
for deb in "$prefix"/debs/*.deb; do
    dpkg -x "$deb" "$prefix/root.new"
done
touch "$prefix/root.new/.complete"
mv "$prefix/root.new" "$prefix/root"
echo "$prefix/root"
