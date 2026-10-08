/* A C library the Nib program links against: it reads the program's
 * arrays through pointers and calls back the rule the program exports,
 * declared with CPoint in the header the compiler generates from main.nib. */

#include "main.h"

unsigned short checksum(const unsigned char *data, unsigned short count)
{
    unsigned short sum = 0;
    while (count--)
        sum = (unsigned short)((sum << 1 | sum >> 15) ^ *data++);
    return sum;
}

long weighted_sum(const short *values, unsigned short count)
{
    long total = 0;
    unsigned short i;
    for (i = 0; i < count; ++i)
        total += weight(values[i]);
    return total;
}

void centroid(const CPoint *points, unsigned short count, CPoint *out)
{
    long x = 0, y = 0;
    unsigned short i;
    for (i = 0; i < count; ++i) {
        x += points[i].x;
        y += points[i].y;
    }
    out->x = (short)(x / count);
    out->y = (short)(y / count);
}
