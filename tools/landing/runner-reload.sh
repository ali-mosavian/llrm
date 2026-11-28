# runner_env_load FILE: FILE (KEY=value lines) sourced and exported when it exists; the runner reads it every pass, so HEAD_MIN_AGE, ALONE_* and LAND_FAST change without a restart.
# A key an earlier load set and FILE no longer has is unset (RUNNER_ENV_KEYS holds the last load's keys).
runner_env_load() {
  [ -f "$1" ] || return 0
  local keys key
  keys=$(sed -n 's/^\(export \)\{0,1\}\([A-Za-z_][A-Za-z0-9_]*\)=.*/\2/p' "$1" | tr '\n' ' ')
  for key in $RUNNER_ENV_KEYS; do
    case " $keys " in *" $key "*) ;; *) unset "$key" ;; esac
  done
  set -a; . "$1"; set +a
  export RUNNER_ENV_KEYS="$keys"
}
# runner_scripts_sha DIR: one hash of the runner and its helpers (not the tests); a different value means the runner re-executes itself at the top of a pass, when no batch is in flight.
runner_scripts_sha() { cat $(ls "$1"/*.sh | grep -v '/test-') | sha1sum | cut -d' ' -f1; }
