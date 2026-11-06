# tooling_current DIR: DIR (the tooling worktree) detached at the fetched origin/$LAND_BRANCH. parity_squash.py loads before its own clean_checkout moves the worktree, so without this each cycle ran the last cycle's tooling.
. "$(dirname "${BASH_SOURCE[0]:-$0}")/branch.sh"
tooling_current() { git -C "$1" fetch -q origin $LAND_BRANCH && git -C "$1" checkout -q --detach origin/$LAND_BRANCH; }
