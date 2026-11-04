#!/bin/sh
# SF_CMP_SPLIT: eq/ne/lt/le are second public names of cmp (one object, one body), gt of ge. Before, each entry was its own object
# jumping to the core: +10 clk and +2 instr per compare at x86 m16, which kept the split off. Host cc, ELF aliases.
cd "$(dirname "$0")/.." || exit 1
T=$(mktemp -d); trap 'rm -r $T' EXIT; rc=0
for w in f32:sf2 f64:df2; do d=${w%:*}; s=${w#*:}
  for f in lib/fp/$d/cmp_*.c; do cc -O2 -fno-builtin -ffp-contract=off -Iinclude '-DSF_NAME(n)=sfb_##n' -DSF_CMP_SPLIT -c $f -o $T/$d-$(basename $f .c).o || exit 2; done
  own() { nm $T/$d-cmp_$1.o | awk '$2 ~ /^[TtWw]$/ {print $3}' | tr '\n' ' '; }
  for e in eq ne lt le; do own cmp | grep -q "sfb_$e$s " || { echo "$d: sfb_$e$s not in cmp object"; rc=1; }; done
  own ge | grep -q "sfb_gt$s " || { echo "$d: sfb_gt$s not in ge object"; rc=1; }
  for e in eq ne lt le gt; do [ -z "$(own $e)" ] || { echo "$d: cmp_$e.o still defines $(own $e)"; rc=1; }; done
  a=$(nm $T/$d-cmp_cmp.o | awk -v e="sfb_eq$s" -v c="sfb_cmp$s" '$3==e{x=$1} $3==c{y=$1} END{print (x==y && x!="")}'); [ "$a" = 1 ] || { echo "$d: eq is not at cmp's address"; rc=1; }
done
[ $rc = 0 ] && echo "cmp aliases ok"; exit $rc
