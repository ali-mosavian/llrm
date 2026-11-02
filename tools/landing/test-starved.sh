#!/usr/bin/env bash
# Regression: a big PR overlapping every small PR was never batched (assembly keeps the first PRs that merge, and a new small one always came first): #5911 waited for ever.
# After STARVE_K cycles out of a batch it is a batch of its own; a PR out for fewer stays in normal order.
here=$(cd "$(dirname "$0")" && pwd); source "$here/starved.sh"
f=$(mktemp); trap 'rm -f "$f" "$f.new"' EXIT; STARVE_K=6
cycle() {  # the assembler stand-in: the big PR conflicts with every small one, so the first small PR is the batch
  local cands=("$@") s; s=$(starved_next "$f" "${cands[@]}")
  if [ -n "$s" ]; then batch=$s; else batch=${cands[0]}; fi
  starved_record "$f" "$batch" "${cands[@]}"
}
picked_at=0
for i in $(seq 1 10); do
  cycle $((100 + i)) $((200 + i)) 9000; [ "$batch" = 9000 ] && { picked_at=$i; break; }
done
[ "$picked_at" = 7 ] || { echo "FAIL: the big PR was batched at cycle '$picked_at', want 7 (after 6 cycles out)"; exit 1; }
grep -q '^9000 ' "$f" && { echo "FAIL: a batched PR kept its count"; exit 1; }
: > "$f"; for i in 1 2 3 4 5; do cycle $((100 + i)) 9000; [ "$batch" = 9000 ] && { echo "FAIL: a PR out for $i < K cycles jumped the queue"; exit 1; }; done
[ -z "$(starved_next "$f" 9000)" ] || { echo "FAIL: a PR out for 5 cycles counted as starved"; exit 1; }
[ -z "$(starved_next /nonexistent 9000)" ] || { echo "FAIL: a missing file made a PR starved"; exit 1; }
grep -q 'starved_next' "$here/queue-runner.sh" && grep -q 'starved_record' "$here/queue-runner.sh" || { echo "FAIL: queue-runner does not age the queue"; exit 1; }
echo PASS starved
