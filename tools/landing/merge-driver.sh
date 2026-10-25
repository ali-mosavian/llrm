#!/usr/bin/env bash
# Once per machine: git merges a tests file two branches appended to with tools/merge_tests_append.py (see .gitattributes). Run from any clone's root.
git config --global merge.tests-append.driver "python3 tools/merge_tests_append.py %O %A %B"
