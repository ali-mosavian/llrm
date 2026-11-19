/* hand toupper against k_toupper, exact result, for c = -2..400, the out-of-range
   forms (returned unchanged) and every byte value. */
int __fastcall__ toupper(int c);
int k_toupper(int c);

static const int odd[] = {0x0161, 0x4141, 0x8000, -159, -32767, 0x7F41};

int main(void)
{
    int c;
    unsigned i;
    for (c = -2; c <= 400; ++c) {
        if (toupper(c) != k_toupper(c))
            return 1;
    }
    for (i = 0; i < sizeof odd / sizeof odd[0]; ++i) {
        c = odd[i];
        if (toupper(c) != k_toupper(c))
            return 2;
    }
    return 0;
}
