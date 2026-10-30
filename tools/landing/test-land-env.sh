#!/usr/bin/env bash
# Regression: land.sh hardcoded ~/scratch/coord-ps, so parallel mergers shared one checkout and target dir.
l=$(dirname "$0")/land.sh
grep -q 'W=${LAND_WORKTREE:-' "$l" && grep -q 'T=${LAND_TARGET_DIR:-' "$l" && echo PASS || { echo "FAIL: land.sh ignores LAND_WORKTREE/LAND_TARGET_DIR"; exit 1; }
