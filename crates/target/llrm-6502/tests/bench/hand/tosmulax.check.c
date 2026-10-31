extern int hand_tosmulax(int a, int b);
extern int k_tosmulax(int a, int b);

/* cc65 compiles a * b, a / b and a % b of 16-bit operands into a call of the routine under test,
   so the reference widens to long (a different runtime routine). */
static int ref(int a, int b)
{
    return (int)((unsigned long)(unsigned)a * (unsigned)b);
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

static int bad(int a, int b)
{
    int h = hand_tosmulax(a, b);
    return h != ref(a, b) || h != k_tosmulax(a, b);
}

int main(void)
{
    unsigned i, r;
    int a, b;
    for (i = 0; i < sizeof tab / sizeof tab[0]; ++i) {
        a = (int)tab[i][0];
        b = (int)tab[i][1];
        if (bad(a, b))
            return i + 1;
    }
    for (i = 0; i < 3000; ++i) {
        a = (int)rnd();
        r = rnd();
        b = (int)((r >> (r & 15)) ^ (rnd() & 0x0F));
        if (bad(a, b))
            return 255;
    }
    return 0;
}
