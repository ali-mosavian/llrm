#!/bin/bash
# Positive control for lsr_shadow.py: the unreduced input (`cat`) is not what the pin's loop-reduce
# prints, so every loop must say DIFFERS. A tool that says "same" here cannot discriminate.
cd "$(dirname "$0")/.."
out=$(python3 tools/lsr_shadow.py tools/pin_llc_loop_reduce.sh /bin/cat --triple arm64-apple-macosx tools/lsr_shadow/*.ll)
differs=$(grep -c ': DIFFERS' <<<"$out")
total=$(ls tools/lsr_shadow/*.ll | wc -l)
[ "$differs" -eq "$total" ] || { echo "lsr_shadow.py cannot discriminate: $differs of $total DIFFERS"; exit 1; }
echo "control ok: $differs of $total DIFFERS"
