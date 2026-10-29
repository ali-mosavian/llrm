#!/bin/sh
# clangfetch.sh NAME DIR: DIR holds the clang tree the toolchain workflow uploaded as artifact NAME.
# A pull request into main reads only main's caches, and the trees are saved on feat/parity: the cache misses, the artifact does not.
set -eu
name=$1
dir=$2
[ -x "$dir/bin/clang" ] && exit 0
url=$(gh api "repos/$GITHUB_REPOSITORY/actions/artifacts?name=$name&per_page=10" --jq '[.artifacts[]|select(.expired|not)][0].archive_download_url')
case "$url" in http*) ;; *) echo "clangfetch: no live artifact $name" >&2; exit 1 ;; esac
tmp=$(mktemp -d)
gh api "$url" >"$tmp/a.zip"
python3 -m zipfile -e "$tmp/a.zip" "$tmp"
mkdir -p "$dir"
zstd -dc "$tmp/tree.tar.zst" | tar -C "$dir" -xf -
rm -rf "$tmp"
