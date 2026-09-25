/* The smallest real WCC capture that distinguishes unsigned scalar semantics. */

unsigned long unsigned_divide(unsigned short value, unsigned long divisor)
{
    return (unsigned long)value / divisor;
}

unsigned short unsigned_less(unsigned short left, unsigned short right)
{
    if (left < right)
        return 1;
    return 0;
}
