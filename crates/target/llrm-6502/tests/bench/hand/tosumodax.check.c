extern unsigned hand_tosumodax(unsigned a, unsigned b);
extern unsigned k_tosumodax(unsigned a, unsigned b);

/* cc65 compiles a * b, a / b and a % b of 16-bit operands into a call of the routine under test,
   so the reference widens to long (a different runtime routine). */
static unsigned ref(unsigned a, unsigned b)
{
    return (unsigned)((unsigned long)a % b);
}

static const unsigned tab[][2] = {
    {0u, 1u},
    {0u, 7u},
    {1u, 1u},
    {7u, 1u},
    {7u, 2u},
    {100u, 10u},
    {100u, 7u},
    {255u, 255u},
    {256u, 255u},
    {255u, 256u},
    {65535u, 1u},
    {65535u, 255u},
    {65535u, 256u},
    {65535u, 65535u},
    {65535u, 2u},
    {60707u, 37u},
    {4829u, 37u},
    {37u, 4829u},
    {4829u, 65531u},
    {60707u, 65499u},
    {30000u, 7u},
    {32767u, 1u},
    {32767u, 32767u},
    {1u, 32767u},
    {40000u, 3u},
    {40000u, 40000u},
    {40000u, 65535u},
    {65535u, 40000u},
    {65535u, 32768u},
    {4660u, 86u},
    {4660u, 342u},
    {65531u, 65531u},
    {65531u, 2u},
    {65531u, 3u},
    {5u, 65535u},
    {65535u, 65531u},
    {65535u, 32769u}
};

static unsigned s = 0xACE1u;
static unsigned rnd(void)
{
    s ^= s << 7;
    s ^= s >> 9;
    s ^= s << 8;
    return s;
}

static int bad(unsigned a, unsigned b)
{
    unsigned h = hand_tosumodax(a, b);
    return h != ref(a, b) || h != k_tosumodax(a, b);
}

int main(void)
{
    unsigned i, r;
    unsigned a, b;
    for (i = 0; i < sizeof tab / sizeof tab[0]; ++i) {
        a = (unsigned)tab[i][0];
        b = (unsigned)tab[i][1];
        if (b == 0)
            continue;
        if (bad(a, b))
            return i + 1;
    }
    for (i = 0; i < 3000; ++i) {
        a = (unsigned)rnd();
        r = rnd();
        b = (unsigned)((r >> (r & 15)) ^ (rnd() & 0x0F));
        if (b == 0)
            continue;
        if (bad(a, b))
            return 255;
    }
    return 0;
}
