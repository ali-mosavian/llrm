// RUN: llrm-cc %s -m64 -O1 -S -o /dev/stdout
// IV users met in descending program order made LSR pick other bases than the pin's use-list order: the two init nests below
// kept a counter in rbx, pushed and popped around the frame (bench/matmul -O1 295 B against 310 B, 282 B -> 256 B here).
// CHECK-LABEL: f proc near
// CHECK-NOT: push rbx
// CHECK: ret
unsigned long f(unsigned short seed)
{
    short a[8][8], b[8][8];
    unsigned short i, j;
    unsigned long s = 0;
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) {
            a[i][j] = (short)(i * 3 + j + 1 + seed);
            b[i][j] = (short)(i == j ? 2 : (i + j) % 3);
        }
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) s += a[i][j] * b[j][i];
    return s;
}
