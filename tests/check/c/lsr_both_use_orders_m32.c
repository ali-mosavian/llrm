// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// DEVIATE LU14 on m32: LSR's search takes the uses in both orders. In the first alone `sum += blk[i][i]` kept a
// pointer and a counter (`inc; add 36; cmp; jne`, 3 loop instructions more bytes), the other order counts to zero
// (x_dct -Os +2 B and +1,088 clk after #9696).
// CHECK-LABEL: _diag proc
// CHECK: add ecx, 36
// CHECK-NEXT: jne
// CHECK-NOT: cmp
// CHECK: _diag endp
int blk[8][8];
long diag(long sum)
{
    int i;
    for (i = 0; i < 8; ++i) sum += blk[i][i];
    return sum;
}
