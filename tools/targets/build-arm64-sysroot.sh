#!/usr/bin/env bash
# Unpacks Ubuntu's arm64 cross libc and libgcc into ~/.cache/llrm-arm64-sysroot (ARM64_SYSROOT overrides) so that
# clang and lld build static arm64 Linux programs that qemu-aarch64 runs. No root: apt-get download and dpkg -x.
# Prints the sysroot's root. Needs clang, ld.lld and qemu-aarch64 on the path.
set -euo pipefail

prefix=${ARM64_SYSROOT:-$HOME/.cache/llrm-arm64-sysroot}
if [ -d "$prefix/root/usr/aarch64-linux-gnu/lib" ]; then
    echo "$prefix/root"
    exit 0
fi
mkdir -p "$prefix/debs"
(cd "$prefix/debs" && apt-get download libc6-dev-arm64-cross libc6-arm64-cross linux-libc-dev-arm64-cross \
    libgcc-s1-arm64-cross libgcc-15-dev-arm64-cross >&2)
for deb in "$prefix"/debs/*.deb; do
    dpkg -x "$deb" "$prefix/root"
done
echo "$prefix/root"
