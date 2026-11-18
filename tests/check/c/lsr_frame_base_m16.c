// RUN: llrm-cc %s -m16 -Os -S -o /dev/stdout
// Two local arrays under one index at -Os in real mode: each is the frame register and a displacement, `[bp+si-136]` and `[bp+si-264]`
// off one counter. LSR took each array's address as a register (two `lea`, one spilled): bench matmul's fill nest, +47 B over the bar.
// CHECK-LABEL: _fill_local@3 proc
// CHECK-NOT: {{lea .*\[bp\+}}
// CHECK: mov word ptr [bp+{{si|di}}-136],
// CHECK: mov word ptr [bp+{{si|di}}-264],
// CHECK: _fill_local@3 endp
unsigned short fill_local(unsigned short seed)
{
    short a[8][8], b[8][8];
    unsigned short i, j;
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) {
            a[i][j] = (short)(i * 3 + j + 1 + seed);
            b[i][j] = (short)(i == j ? 2 : (i + j) % 3);
        }
    return a[3][4] + b[5][6];
}
