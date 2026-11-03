#!/bin/sh
# Control for reach65.py: root calls b by jsr; c is unreferenced and must not count.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d)
mk() { cat > $T/t.s <<E2
.export root, b, tail, c, end
root: jsr b
      rts
b:    lda #1
      sta \$10
      .export c
tail: rts
c:    lda #2
      ldx #3
      rts
end: rts
E2
  ca65 -t sim6502 $T/t.s -o $T/t.o; printf 'MEMORY { M: start=$200, size=$1000; }\nSEGMENTS { CODE: load=M, type=ro; }\n' > $T/t.cfg
  ld65 -C $T/t.cfg -Ln $T/l.txt $T/t.o -o $T/t.bin; python3 reach65.py $T/t.s $T/l.txt root | cut -d' ' -f1; }
a=$(mk "")
# root = jsr b + rts = 4 B, b = 4 B falling through into tail = 1 B, c = 5 B (unreferenced); calling c from root adds 3 B to root and c
[ "$a" = 9 ] || { echo "FAIL reached $a, want 9"; exit 1; }
sed -i 's/^root: jsr b/root: jsr b\n      jsr c/' $T/t.s; ca65 -t sim6502 $T/t.s -o $T/t.o; ld65 -C $T/t.cfg -Ln $T/l.txt $T/t.o -o $T/t.bin
r=$(python3 reach65.py $T/t.s $T/l.txt root | cut -d' ' -f1)
[ "$r" = 17 ] || { echo "FAIL with c referenced $r, want 17"; exit 1; }
echo "ok: reached 9 B (b falls through to tail), 17 B when c is referenced"; rm -r $T
