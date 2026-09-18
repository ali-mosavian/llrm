/* A constant returned by one private call must specialize the next private call. */
static short seed(void)
{
    return 4;
}

static short choose(short value)
{
    if (value == 0) return 1;
    if (value == 1) return 3;
    if (value == 2) return 5;
    if (value == 3) return 7;
    if (value == 4) return 11;
    if (value == 5) return 13;
    if (value == 6) return 17;
    if (value == 7) return 19;
    if (value == 8) return 23;
    return (short)(value + 29);
}

short chainedConstant(void)
{
    return choose(seed());
}
