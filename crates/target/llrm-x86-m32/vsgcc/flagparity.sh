#!/bin/bash
# flagparity.sh PROG [LEVEL...]: ours against the gcc and clang columns, flags read from the files the table runs.
# Logs the clang line llrm-cc ran (a wrapper in $LLRM_PIN_CLANG), then flagcheck.py compares it with build.sh's gcc and clang lines.
# Prints one verdict per level: SAME, or the difference (exit 2).
set -e
P=$1; shift; LEVELS=${*:-O1 O2 O3 Os}
D=$(cd "$(dirname "$0")" && pwd); R=$(cd "$D/../../../.." && pwd)
O=${VSGCC_WORK:-$HOME/scratch/vsgcc-work}; mkdir -p $O/b
LLRM=${LLRM:-$(python3 "$R/tools/llrmbin.py" bin)/llrm-cc}
REAL=${REAL_PIN_CLANG:-$(python3 "$R/tools/pin_clang.py")}
printf '#!/bin/bash\necho "$@" >> %s/ours.clang.log\nexec %s "$@"\n' "$O" "$REAL" > $O/clanglog.sh; chmod +x $O/clanglog.sh
python3 "$D/wrap.py" $P "$([ -d $D/kernels/$P ] && echo $D/kernels/$P/$P.c || echo $R/bench/$P/$P.c)" > $O/b/$P.c
F="-m32 -march=i486 -Dfar= $(python3 "$R/tools/external_flags.py" --reloc=-fno-pic)"
rc=0
for o in $LEVELS; do
  rm -f $O/ours.clang.log
  LLRM_PIN_CLANG=$O/clanglog.sh $LLRM --target=x86_32-wat -mabi=sysv -mlong-double-80 -$o -march=i486 -c -o /dev/null $O/b/$P.c >/dev/null 2>&1 || true
  LINE=$(grep -e '-emit-llvm' $O/ours.clang.log | head -1)
  [ -n "$LINE" ] || { echo "$o: no front-end line logged (nothing compared)"; rc=2; continue; }
  for c in "gcc -mpreferred-stack-boundary=2" "clang -mstack-alignment=4"; do
    set -- $c
    if python3 "$D/flagcheck.py" ours $o "$LINE" $F $2 -$o -mlong-double-80; then echo "$o $1: SAME"; else rc=2; fi
  done
done
exit $rc
