extern unsigned hand_aslax3(unsigned x);
extern unsigned k_aslax3(unsigned x);

/* cc65 compiles a constant shift of a 16-bit value into a call of the routine under test,
   so the reference widens to long. */
static unsigned ref(unsigned x)
{
    return (unsigned)(((unsigned long)x << 3) & 0xFFFFu);
}

int main(void)
{
    unsigned i = 0;
    do {
        unsigned x = (unsigned)i;
        unsigned h = hand_aslax3(x);
        if (h != ref(x) || h != k_aslax3(x))
            return i < 255 ? i + 1 : 255;
    } while (++i);
    return 0;
}
