#!/usr/bin/env bash
# prune-target.sh DIR [--apply]: removes stale cargo output from a persistent target dir: in every PROFILE/deps, the files of each unit (name before
# `-<16 hex>`) whose hash is not the unit's newest, when they are older than AGE_MIN; and PROFILE/incremental entries older than AGE_MIN (the landing builds with CARGO_INCREMENTAL=0).
# A unit with one hash is never touched, so dependencies that did not change survive. Dry run unless --apply; prints the size.
dir=${1:?target dir}; apply=$2; age=${AGE_MIN:-180}
find "$dir"/*/deps -maxdepth 1 -type f -printf '%T@ %s %p\n' 2>/dev/null | awk -v age="$age" -v now="$(date +%s)" '
  { n=split($3, parts, "/"); base=parts[n]; if (!match(base, /-[0-9a-f]{16}/)) next
    unit=substr(base,1,RSTART-1); hash=substr(base,RSTART+1,16); key=$3; sub(/[^\/]*$/,"",key); key=key unit
    if (!(key in newest) || $1 > newest[key]) { newest[key]=$1; newhash[key]=hash }
    files[NR]=$0; fkey[NR]=key; fhash[NR]=hash }
  END { for (i in files) { split(files[i], f, " "); if (fhash[i] != newhash[fkey[i]] && now - f[1] > age*60) print f[2], f[3] } }' > "${TMPDIR:-/tmp}/prune-list.$$"
find "$dir"/*/incremental -mindepth 1 -maxdepth 1 -mmin +"$age" -print0 2>/dev/null | while IFS= read -r -d '' p; do echo "$(( $(du -sk "$p" | cut -f1) * 1024 )) $p"; done >> "${TMPDIR:-/tmp}/prune-list.$$"
awk '{s+=$1} END{printf "%.1f GB (%d bytes) in %d entries\n", s/1e9, s, NR}' "${TMPDIR:-/tmp}/prune-list.$$"
if [ "$apply" = --apply ]; then { echo "$(date +%H:%M:%S) pruned $(wc -l < "${TMPDIR:-/tmp}/prune-list.$$") entries"; cat "${TMPDIR:-/tmp}/prune-list.$$"; } >> "${PRUNE_LOG:-/dev/null}"; cut -d' ' -f2- "${TMPDIR:-/tmp}/prune-list.$$" | tr '\n' '\0' | xargs -0 rm -rf --; fi
rm -f "${TMPDIR:-/tmp}/prune-list.$$"
