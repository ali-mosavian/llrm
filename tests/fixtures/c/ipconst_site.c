/* One constant call may specialize without changing a dynamic caller. */
static short adjust(short value)
{
    if (value == 0) return 7;
    return (short)(value + 3);
}

short zeroAdjusted(void)
{
    return adjust(0);
}

short dynamicAdjusted(short value)
{
    return adjust(value);
}
