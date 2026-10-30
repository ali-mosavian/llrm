/* report(long): the DOS OS layer prints a signed decimal and a newline; the host reference does the same. */
#ifndef __DOS__
#include <stdio.h>
static void report(long value) { printf("%ld\n", value); }
#else
void report(long);
#endif
