#!/usr/bin/env bash
# Once per clone (its own .git, nothing account-wide): git merges a tests file two branches appended to with tools/merge_tests_append.py. Run from the clone's root.
# Git reads attributes from the branch being merged INTO, so a branch that predates .gitattributes' `tests-append` still said `union` when it merged parity (#4567,
# #4573); info/attributes outranks the tree's .gitattributes. Without the driver configured git conflicts on such a file, as it always did.
set -euo pipefail
git config --local merge.tests-append.driver "python3 tools/merge_tests_append.py %O %A %B"
attributes=$(git rev-parse --git-path info/attributes)
mkdir -p "$(dirname "$attributes")"
for line in '*_tests.rs merge=tests-append' 'tests.rs merge=tests-append'; do
  grep -qxF "$line" "$attributes" 2>/dev/null || echo "$line" >> "$attributes"
done
