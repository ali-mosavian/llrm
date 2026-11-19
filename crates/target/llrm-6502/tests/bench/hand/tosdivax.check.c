extern int hand_tosdivax(int a, int b);
extern int k_tosdivax(int a, int b);

/* cc65 compiles a * b, a / b and a % b of 16-bit operands into a call of the routine under test,
   so the reference widens to long (a different runtime routine). */
static int ref(int a, int b)
{
    return (int)((long)a / (long)b);
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

static int bad(int a, int b)
{
    int h = hand_tosdivax(a, b);
    return h != ref(a, b) || h != k_tosdivax(a, b);
}

int main(void)
{
    unsigned i, r;
    int a, b;
    for (i = 0; i < sizeof tab / sizeof tab[0]; ++i) {
        a = (int)tab[i][0];
        b = (int)tab[i][1];
        if (b == 0 || (unsigned)a == 0x8000u || (unsigned)b == 0x8000u)
            continue;
        if (bad(a, b))
            return i + 1;
    }
    for (i = 0; i < 3000; ++i) {
        a = (int)rnd();
        r = rnd();
        b = (int)((r >> (r & 15)) ^ (rnd() & 0x0F));
        if (b == 0 || (unsigned)a == 0x8000u || (unsigned)b == 0x8000u)
            continue;
        if (bad(a, b))
            return 255;
    }
    return 0;
}
