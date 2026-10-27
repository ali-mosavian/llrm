# runner_env_load FILE: FILE (KEY=value lines) sourced and exported when it exists; the runner reads it every pass, so HEAD_MIN_AGE, ALONE_* and LAND_FAST change without a restart.
runner_env_load() { [ -f "$1" ] || return 0; set -a; . "$1"; set +a; }
# runner_scripts_sha DIR: one hash of the runner and its helpers (not the tests); a different value means the runner re-executes itself at the top of a pass, when no batch is in flight.
runner_scripts_sha() { cat $(ls "$1"/*.sh | grep -v '/test-') | sha1sum | cut -d' ' -f1; }
