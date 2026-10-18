#!/usr/bin/env bash
# Before every push: the gate's cheap checks (fmt, generated docs, no_local_paths, target_facts, the source scans) in about a minute.
# One verdict line first. Not the full gate: see tools/gate/prepush.py.
exec python3 "$(dirname "$0")/gate/prepush.py" "$@"
