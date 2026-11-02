// RUN: llrm-cc %s {-O2 | -Os} -march=i486 -S -o /dev/stdout
// The row index `(y + 61) & 63` is a 6-bit counter after lsr; maskscale merges it with the trip counter into one byte-offset
// counter (LP-7: no pin pass). One `and` reads it, and the exit tests it: no movzx/shl per row, no second counter.
// CHECK-LABEL: {{_rows(@3)?}} proc
// CHECK: and {{[a-d]x}}, 8064
// CHECK: cmp {{[a-d]x}}, 11008
// CHECK-NOT: movzx
// CHECK-NOT: shl
// CHECK: {{_rows(@3)?}} endp
short tile[64][64];

long rows(void)
{
    long total = 0;
    short x, y;

    for (y = 0; y < 25; ++y)
        for (x = 0; x < 40; ++x)
            total += tile[(y + 61) & 63][(x + 7) & 63];
    return total;
}
