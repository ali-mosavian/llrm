// RUN: llrm-cc %s -m16 -Os -S -o /dev/stdout
// matmul's fill nest at -Os in real mode: LSR's winner-register narrowing (past 65535
// solutions) put `a[i][j]` and `b[i][j]` on one `j` scaled by 2, and a scaled index is an
// instruction here (`lea bx,[cx+cx]` per trip, row bases hoisted into registers that
// spill). Searched whole, both stores take the same step-2 index: +19 B and +4% clk on
// bench matmul.
// CHECK-LABEL: _nest@3 proc
// CHECK: mov word ptr [bp+di-{{[0-9]+}}], {{[a-z]+}}
// CHECK: mov word ptr [bp+di-{{[0-9]+}}], {{[a-z]+}}
// CHECK-NOT: lea {{[a-z]+}}, [{{[a-z]+}}+{{[a-z]+}}]
// CHECK: _nest@3 endp
unsigned long nest(unsigned short seed)
{
    short a[8][8], b[8][8];
    unsigned short i, j;
    unsigned long s = 0;
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) {
            a[i][j] = (short)(i * 3 + j + 1 + seed);
            b[i][j] = (short)(i == j ? 2 : (i + j) % 3);
        }
    for (i = 0; i < 8; ++i) s += a[i][i] + b[i][7 - i];
    return s;
}
