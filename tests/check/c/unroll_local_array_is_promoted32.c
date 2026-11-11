// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 -S -o /dev/stdout
// A local array filled by one 12-trip loop and read by the next is promoted to registers once both are unrolled: no array, no loop (tests/run frameless: +160 B when only the byte price of the copy was counted, UP1).
// CHECK-LABEL: _mixed proc
// CHECK-NOT: jne
// CHECK: _mixed endp
extern long add4(long a, long b, long c, long d);
long mixed(long n)
{
    long local[12];
    long s = 0;
    int i;
    for (i = 0; i < 12; ++i) local[i] = n + i;
    for (i = 0; i < 12; ++i) s += add4(local[i], local[11 - i], n, i);
    return s;
}
