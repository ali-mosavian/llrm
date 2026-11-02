extern unsigned hand_umul8x8r16(unsigned char a, unsigned char b);
extern unsigned k_umul8x8r16(unsigned char a, unsigned char b);

static const unsigned char in[] = {0, 1, 2, 3, 127, 128, 129, 200, 254, 255};

int main(void)
{
    unsigned i, j, n = 0, want;
    for (i = 0; i < sizeof in; ++i)
        for (j = 0; j < sizeof in; ++j) {
            ++n;
            want = in[i] * in[j];
            if (hand_umul8x8r16(in[i], in[j]) != want || k_umul8x8r16(in[i], in[j]) != want)
                return n;
        }
    return 0;
}
