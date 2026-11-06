extern unsigned hand_tosumulax(unsigned a, unsigned b);
extern unsigned k_tosumulax(unsigned a, unsigned b);

/* cc65 compiles a * b, a / b and a % b of 16-bit operands into a call of the routine under test,
   so the reference widens to long (a different runtime routine). */
static unsigned ref(unsigned a, unsigned b)
{
    return (unsigned)((unsigned long)a * b);
}

static const unsigned tab[][2] = {
    {0u, 0u},
    {0u, 5u},
    {5u, 0u},
    {1u, 1u},
    {1u, 40000u},
    {40000u, 1u},
    {255u, 255u},
    {256u, 256u},
    {255u, 257u},
    {257u, 255u},
    {300u, 200u},
    {200u, 300u},
    {1234u, 567u},
    {567u, 1234u},
    {65535u, 65535u},
    {65535u, 1u},
    {32768u, 2u},
    {32768u, 32768u},
    {12345u, 54321u},
    {255u, 65280u},
    {65280u, 255u},
    {3u, 21846u},
    {65535u, 2u},
    {7u, 9362u}
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
    unsigned h = hand_tosumulax(a, b);
    return h != ref(a, b) || h != k_tosumulax(a, b);
}

int main(void)
{
    unsigned i, r;
    unsigned a, b;
    for (i = 0; i < sizeof tab / sizeof tab[0]; ++i) {
        a = (unsigned)tab[i][0];
        b = (unsigned)tab[i][1];
        if (bad(a, b))
            return i + 1;
    }
    for (i = 0; i < 3000; ++i) {
        a = (unsigned)rnd();
        r = rnd();
        b = (unsigned)((r >> (r & 15)) ^ (rnd() & 0x0F));
        if (bad(a, b))
            return 255;
    }
    return 0;
}
