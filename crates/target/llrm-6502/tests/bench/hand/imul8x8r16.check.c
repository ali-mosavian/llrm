extern int hand_imul8x8r16(signed char a, signed char b);
extern int k_imul8x8r16(signed char a, signed char b);

static const signed char in[] = {0, 1, -1, 2, -2, 3, 100, -100, 127, -128, -127, 64, -64};

int main(void)
{
    unsigned i, j, n = 0;
    int want;
    for (i = 0; i < sizeof in; ++i)
        for (j = 0; j < sizeof in; ++j) {
            ++n;
            want = (int)in[i] * (int)in[j];
            if (hand_imul8x8r16(in[i], in[j]) != want || k_imul8x8r16(in[i], in[j]) != want)
                return n > 255 ? 255 : n;
        }
    return 0;
}
