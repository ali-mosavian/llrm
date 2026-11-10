// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// The -Os price of a variable-length copy (the byte loop is smaller than the call) also held at -O2, where bytes are
// not the whole cost: the loop stayed and x_radix -O2 went 31627 -> 37249 clocks for 287 B less, past gcc.
extern unsigned char src[12000];
unsigned char dst[512];
long pos;
int rd(int count)
{
    int n = 0;
    while (n < count && pos < 12000) dst[n++] = src[pos++];
    return n;
}
// MEM-1 (#7233) lowers the memcpy to an inline `rep movsd` at speed; the byte loop must not come back.
// CHECK-LABEL: rd proc
// CHECK: rep movsd
