/* hand isalpha against k_isalpha for c = -2..400, the 0x100-multiple and sign-bit
   out-of-range forms, and every byte value; compares truthiness only
   (the hand routine returns a class mask). */
int __fastcall__ isalpha(int c);
int k_isalpha(int c);

static const int odd[] = {0x0161, 0x4141, 0x8000, -159, -32767, 0x7F41};

int main(void)
{
    int c;
    unsigned i;
    for (c = -2; c <= 400; ++c) {
        if (!isalpha(c) != !k_isalpha(c))
            return 1;
    }
    for (i = 0; i < sizeof odd / sizeof odd[0]; ++i) {
        c = odd[i];
        if (!isalpha(c) != !k_isalpha(c))
            return 2;
    }
    return 0;
}
