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
if [ -d "$prefix/root/usr/aarch64-linux-gnu/lib" ]; then
    echo "$prefix/root"
    exit 0
fi
libgcc=$(apt-cache pkgnames libgcc- | newest_libgcc)
[ -n "$libgcc" ] || { echo "no libgcc-N-dev-arm64-cross package in apt" >&2; exit 1; }
mkdir -p "$prefix/debs"
(cd "$prefix/debs" && apt-get download libc6-dev-arm64-cross libc6-arm64-cross linux-libc-dev-arm64-cross \
    libgcc-s1-arm64-cross "$libgcc" >&2)
for deb in "$prefix"/debs/*.deb; do
    dpkg -x "$deb" "$prefix/root"
done
echo "$prefix/root"
