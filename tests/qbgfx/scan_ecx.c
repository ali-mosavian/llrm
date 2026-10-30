/* A CGA row of one colour has no pixel of another, wherever C left the upper half of ecx:
   B$FSCC counted the bytes of the row in all of ecx.  Prints 0, or the `found` it gave. */
#include "device.h"

extern void report(long value);
extern void dirty_scan(FillScan *scan);
#pragma aux dirty_scan "B$DIRTY"

static unsigned char row[96];

int main(void)
{
    FillScan scan;
    int i;

    for (i = 0; i < 80; i++)
        row[i] = 0;
    for (i = 80; i < 96; i++)
        row[i] = 0xFF;
    scan.at = row;
    scan.x = 0;
    scan.to = 80 * 4 - 1;
    scan.c1 = 0;
    scan.c2 = 0;
    scan.flags = 4;
    dirty_scan(&scan);
    report((long)scan.found + 1);
    return 0;
}
