# Sourced. The toolchain's trees live under ~/.cache/llrm, shared by every
# worktree and every cargo build running at once.
#
# claimed FILE: this process made FILE, holding its pid; one concurrent caller does,
# the rest do not. Made by the shell with O_EXCL (noclobber), not by mkdir: uutils'
# mkdir lets two concurrent callers both succeed.
claimed() {
    (set -C; echo $$ >"$1") 2>/dev/null
}

# cached DIR PRODUCER [ARG...]: DIR holds what PRODUCER made, complete or not
# at all. DIR is named for what it was made from (a commit, a hash), never
# changed after, and valid only once DIR/.complete exists, which is written
# last. One process produces at a time (a lock file holding its owner's pid
# so a dead owner's lock is taken over, once it has stayed the same for a second);
# the others wait and use its result.
# PRODUCER runs as `PRODUCER ARG... DIR`, DIR being empty.
cached() {
    dir=$1
    shift
    [ -f "$dir/.complete" ] && return 0
    mkdir -p "$(dirname "$dir")"
    lock="$dir.lock"
    until claimed "$lock"; do
        [ -f "$dir/.complete" ] && return 0
        owner=$(cat "$lock" 2>/dev/null || true)
        if [ -n "$owner" ] && ! kill -0 "$owner" 2>/dev/null; then
            # Dead, and still the same lock a second on: an owner that finished
            # has removed its lock and another may hold a new one.
            sleep 1
            if [ "$(cat "$lock" 2>/dev/null || true)" = "$owner" ] && [ ! -f "$dir/.complete" ]; then
                rm -f "$lock"
            fi
        fi
        sleep 1
    done
    made=0
    if [ ! -f "$dir/.complete" ]; then
        rm -rf "$dir"
        mkdir -p "$dir"
        if "$@" "$dir"; then
            touch "$dir/.complete"
        else
            made=1
            rm -rf "$dir"
        fi
    fi
    rm -f "$lock"
    return $made
}
