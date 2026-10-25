#!/usr/bin/env bash
# Once per clone (the repository's own .git/config, nothing account-wide): git merges a tests file two branches appended to with tools/merge_tests_append.py (see
# .gitattributes). Run from the clone's root. Without it git conflicts on such a file, as it always did.
git config --local merge.tests-append.driver "python3 tools/merge_tests_append.py %O %A %B"
