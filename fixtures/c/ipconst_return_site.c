/* A returned constant may specialize one call while another stays dynamic. */
static short seed(void)
{
    return 4;
}

static short choose(short value)
{
    if (value == 4) return 11;
    return (short)(value + 29);
}

short dynamicChoose(short value)
{
    return choose(value);
}

short returnedConstantChoose(void)
{
    return choose(seed());
}
