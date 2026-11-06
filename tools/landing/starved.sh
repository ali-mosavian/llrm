# Aging: a PR left out of K consecutive batches becomes a batch of its own, as urgent.txt does. Without it a big PR that overlaps every small one is never picked.
# Counts live in FILE ("PR count" lines); each call is one awk pass over FILE and the candidates, O(queue).
STARVE_K=${STARVE_K:-6}
# starved_next FILE CAND...: the first CAND (queue order) whose count reached STARVE_K.
starved_next() {
  local file=$1; shift
  printf '%s\n' "$@" | awk -v k="$STARVE_K" 'NR==FNR { c[$1]=$2; next } ($1 in c) && c[$1] >= k { print $1; exit }' "$file" - 2>/dev/null
}
# starved_record FILE "BATCH" CAND...: one cycle done. A candidate left out of BATCH gains 1; a batched one is forgotten; PRs not candidates this cycle keep their count.
starved_record() {
  local file=$1 batch=$2; shift 2
  [ -f "$file" ] || : > "$file"
  { cat "$file"; echo "---"; printf '%s\n' "$@"; } | awk -v b=" $batch " '
    !seen_sep && $1 == "---" { seen_sep=1; next }
    !seen_sep { c[$1]=$2; next }
    { n=$1; if (index(b, " " n " ")) delete c[n]; else c[n]=c[n]+1 }
    END { for (n in c) print n, c[n] }' > "$file.new" && mv "$file.new" "$file"
}
