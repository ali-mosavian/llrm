/* Integer to text (QB rt/ifout.asm B$FOUTBX for VT_I2 and VT_I4). */
#include "fout.h"

word fout_i4(long v, byte *out)
{
    unsigned long u = v < 0 ? -(unsigned long)v : (unsigned long)v;
    byte digits[10];
    word n = 0, len = 1;

    do {
        digits[n++] = '0' + (byte)(u % 10);
        u /= 10;
    } while (u);
    out[0] = v < 0 ? '-' : ' ';
    while (n)
        out[len++] = digits[--n];
    return len;
}
