/* Integer to text (QB rt/ifout.asm, B$FOUTBX for VT_I2 and VT_I4). */
#include "fout.h"

word fout_i4(
    long v,
    char *out
)
{
    unsigned long magnitude = v < 0 ? -(unsigned long)v : (unsigned long)v;
    char digits[10];
    word count = 0, length = 1;

    do {
        digits[count++] = '0' + magnitude % 10;
        magnitude /= 10;
    } while (magnitude);
    out[0] = v < 0 ? '-' : ' ';
    while (count)
        out[length++] = digits[--count];
    return length;
}
