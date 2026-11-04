# serial_batch PR...: the PRs to land this pass: only the first when SERIAL=1 (runner.env), one squash commit per PR; all of them otherwise.
serial_batch() { if [ "${SERIAL:-}" = 1 ]; then echo "${1:-}"; else echo "$@"; fi; }
