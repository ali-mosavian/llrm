#!/bin/bash
# precheck.sh [PR] [--body-file FILE --title TEXT]: what the landing runner refuses a PR for, asked before marking it ready (run in the PR's worktree).
# Every failure is named; exit 1 if any failed, and a PR that fails stays draft. See tools/precheck_pr.py.
cd "$(dirname "$0")/../.." && exec uv run -q --project tools python tools/precheck_pr.py "$@"
