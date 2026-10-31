extern unsigned hand_mulax5(unsigned x);
extern unsigned k_mulax5(unsigned x);

static const unsigned in[] = {0, 1, 2, 255, 256, 257, 0x7FFF, 0x8000, 0x8001, 0xFFFF, 0xFFFE,
                              0x1234, 0x5555, 0xAAAA, 0xFF00, 1234};

int main(void)
{
    unsigned i, x;
    for (i = 0; i < sizeof in / sizeof in[0]; ++i) {
        x = in[i];
        if (hand_mulax5(x) != x * 5u || k_mulax5(x) != x * 5u)
            return i + 1;
    }
    for (x = 0; x < 0xFFFFu - 96; x += 97)
        if (hand_mulax5(x) != x * 5u || k_mulax5(x) != x * 5u)
            return 255;
    return 0;
}
