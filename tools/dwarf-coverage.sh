#!/usr/bin/env bash
# dwarf-coverage.sh llrm-c: llvm-dwarfdump --statistics of every bench C program built by llrm-c and by gcc, per level:
# source variables with a location, and the share of their scope bytes a location covers. The scoreboard for -g's coverage.
C=$1; D=${DWARFDUMP:-llvm-dwarfdump-20}; T=$(mktemp -d)
for o in O1 O2 Os; do
  for who in llrm gcc; do tot=0; loc=0; cov=0; par=0
    for f in bench/*/*.c; do n=$(basename "$f" .c); obj=$T/$who.$n.o
      if [ $who = llrm ]; then $C -m32 -fobject-format=elf -$o -g "$f" -o "$obj" >/dev/null 2>&1 || continue
      else gcc -m32 -$o -g -fno-pie -c "$f" -o "$obj" >/dev/null 2>&1 || continue; fi
      s=$($D --statistics "$obj" 2>/dev/null | python3 -c 'import json,sys; j=json.load(sys.stdin); print(j["#source variables"], j["#source variables with location"], j["sum_all_variables(#bytes in any scope covered by DW_AT_location)"], j["sum_all_variables(#bytes in parent scope)"])') || continue
      set -- $s; tot=$((tot+$1)); loc=$((loc+$2)); cov=$((cov+$3)); par=$((par+$4))
    done
    echo "$o $who: variables $tot, with location $loc ($((100*loc/tot))%), scope bytes covered $((100*cov/par))%"
  done
done
rm -rf "$T"
