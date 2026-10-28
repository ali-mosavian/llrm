// RUN: llrm-cc %s -m16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// `zext i6` of an `i8` register was masked in MIR (`and bl, 63; movzx bx, bl`, a copy of the live register first) by the llrm-only odd-width pass;
// the DAG type legalizer extends first, as the pin: `movzx bx, dl; and bx, 63` (tile.c, one instruction fewer).
short tile[64][64];
long f(short dy)
{
    long total = 0;
    short x, y;

    for (y = 0; y < 25; ++y)
        for (x = 0; x < 40; ++x)
            total += tile[(y + dy) & 63][x];
    return total;
}
// CHECK-LABEL: _f@3 proc
// CHECK-NOT: and bl, 63
// CHECK: and {{[a-d]x}}, 63
// CHECK: _f@3 endp
