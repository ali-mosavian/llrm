static unsigned char data[4096];
static short counts[256];

void histogram(short *seed)
{
    short index;

    for (index = 0; index < 4096; ++index)
        ++counts[(data[index] * 7 + *seed) & 255];
}
