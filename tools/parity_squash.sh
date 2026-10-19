#!/bin/sh
exec python3 "$(dirname "$0")/parity_squash.py" merge "$@"
