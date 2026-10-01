# Sourced. The toolchain's trees live under ~/.cache/llrm, shared by every
# worktree and every cargo build running at once.
#
# cached DIR PRODUCER [ARG...]: DIR holds what PRODUCER made, complete or not
# at all. DIR is named for what it was made from (a commit, a hash), never
# changed after, and valid only once DIR/.complete exists, which is written
# last. One process produces at a time (a lock directory, with its owner's pid
# so a dead owner's lock is taken over); the others wait and use its result.
# PRODUCER runs as `PRODUCER ARG... DIR`, DIR being empty.
cached() {
    dir=$1
    shift
    [ -f "$dir/.complete" ] && return 0
    mkdir -p "$(dirname "$dir")"
    lock="$dir.lock"
    until mkdir "$lock" 2>/dev/null; do
        [ -f "$dir/.complete" ] && return 0
        owner=$(cat "$lock/pid" 2>/dev/null || true)
        if [ -n "$owner" ] && ! kill -0 "$owner" 2>/dev/null; then
            rm -rf "$lock"
        fi
        sleep 1
    done
    echo $$ >"$lock/pid"
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
    rm -rf "$lock"
    return $made
}
