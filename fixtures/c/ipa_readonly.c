/* A nonvolatile static read has no observable effect when its result is unused. */
static short sampledValue = 41;

static short sample(void)
{
    return sampledValue;
}

short discardSample(void)
{
    sample();
    return 7;
}

static volatile short observedValue;

static short sampleVolatile(void)
{
    return observedValue;
}

short keepVolatileSample(void)
{
    sampleVolatile();
    return 7;
}
