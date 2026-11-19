extern unsigned __fastcall__ mul20(unsigned char value);
extern unsigned k_mul20(unsigned char x);

int main(void)
{
    unsigned i;
    for (i = 0; i < 256; ++i)
        if (mul20((unsigned char)i) != i * 20u || k_mul20((unsigned char)i) != i * 20u)
            return i + 1 > 255 ? 255 : i + 1;
    return 0;
}
