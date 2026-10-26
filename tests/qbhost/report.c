/* What tests/qbgfx's programs call to print their one result, on a host. */
#include <stdio.h>

void report(long value)
{
    printf("%ld\n", value);
}
