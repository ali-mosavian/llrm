# cached_state N FIELD: the field of PR N from the cycle's one GraphQL fill ($LAND_TMP/pr-state.json, pr_state.py); empty when there is none or it is old, and the caller then asks GitHub itself.
cached_state() { [ -f "${HERE:-}/pr_state.py" ] && python3 "$HERE/pr_state.py" cached "$1" "$2" 2>/dev/null; return 0; }
