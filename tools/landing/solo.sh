# solo_listed FILE N: prints N when FILE (one PR number per line, # comments) lists it: that PR is a batch of its own
solo_listed() { grep -v '^#' "$1" 2>/dev/null | sed 's/[[:space:]].*//' | grep -qxF -- "$2" && echo "$2"; }
