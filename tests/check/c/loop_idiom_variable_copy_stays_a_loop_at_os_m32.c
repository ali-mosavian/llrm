// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// A byte copy of a length known only at run time became `memcpy` at m32 -Os: the call, its three pushes, the cleanup
// and the count and exit-value arithmetic, 163 B against gcc's 121 for the loop (x_jpeg_fixed `next`, +26 B). The
// table priced it as an inline `rep movs` (`copy_variable = 0`); it is a call, so the loop stays where it is smaller.
extern unsigned char src[12000];
unsigned char dst[512];
long pos;
int rd(int count)
{
    int n = 0;
    while (n < count && pos < 12000) dst[n++] = src[pos++];
    return n;
}
// CHECK-LABEL: rd proc
// CHECK-NOT: call memcpy
