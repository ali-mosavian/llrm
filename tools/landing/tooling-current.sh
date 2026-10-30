# tooling_current DIR: DIR (the tooling worktree) detached at the fetched origin/feat/parity. parity_squash.py loads before its own clean_checkout moves the worktree, so without this each cycle ran the last cycle's tooling.
tooling_current() { git -C "$1" fetch -q origin feat/parity && git -C "$1" checkout -q --detach origin/feat/parity; }
