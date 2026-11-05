#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc [MOS_RT=mos_rt.s] tests/asm6502/shift64_test.sh -> the 6502 i64 shift libcalls against lib/int/int64.c on sim65: ten counts, three shifts, the exit code is
# a byte fold of the results and must equal the host's. A llrm-cc that passes the libcall count as an i8 (before #7392) against the helper's C `int n` makes the helper read a garbage high byte: exit 0, host 44.
set -e
cd "$(dirname "$0")/../.."; T=$(mktemp -d); trap 'rm -r $T' EXIT
MOS_RT=${MOS_RT:-$PWD/../../crates/target/llrm-6502/runtime/mos_rt.s}; INC="-include $PWD/config/6502/config.h"
cat > $T/m.c <<'C'
typedef unsigned long long u64; typedef long long i64;
volatile unsigned char cnt[10] = {0, 1, 7, 8, 17, 31, 32, 33, 45, 63};
volatile unsigned char sink;
int main(void) {
    unsigned char a = 0, k, j; u64 x = 0x8123456789ABCDEFull, r[3];
    for (k = 0; k < 10; k++) {
        unsigned char n = cnt[k];
        r[0] = x << n; r[1] = x >> n; r[2] = (u64)((i64)x >> n);
        for (j = 0; j < 3; j++) { u64 v = r[j]; unsigned char b; for (b = 0; b < 8; b++) { a = (unsigned char)(a * 31 + (unsigned char)v); v >>= 8; } }
    }
    sink = a;
    return a;
}
C
cc -O0 -o $T/host $T/m.c && { $T/host; want=$?; } || want=$?
$LLRM_CC --target=6502-sim65 -O2 -S -w $T/m.c -o $T/m.s
$LLRM_CC --target=6502-sim65 -Os -S -DSF_INTABI $INC -Iinclude lib/int/int64.c -o $T/i.s
{ grep -h '^rc[0-9]* = ' $T/m.s | sort -u; printf '.export _main\n_main:\n  jsr main\n  rts\n'; grep -v '^rc[0-9]* = ' $T/m.s; grep -v '^rc[0-9]* = ' $T/i.s; cat $MOS_RT; } > $T/p.s
cl65 -t sim6502 -o $T/p.bin $T/p.s
set +e; sim65 -c -x 500000000 $T/p.bin > /dev/null 2>&1; got=$?; set -e
echo "i64 shifts on sim65: exit $got, host $want"
[ "$got" = "$want" ] || { echo "FAIL: the shift helpers disagree with the host"; exit 1; }
