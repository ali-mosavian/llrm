extern unsigned __fastcall__ mul40(unsigned char value);
extern unsigned k_mul40(unsigned char x);

int main(void)
{
    unsigned i;
    for (i = 0; i < 256; ++i)
        if (mul40((unsigned char)i) != i * 40u || k_mul40((unsigned char)i) != i * 40u)
            return i + 1 > 255 ? 255 : i + 1;
    return 0;
}
