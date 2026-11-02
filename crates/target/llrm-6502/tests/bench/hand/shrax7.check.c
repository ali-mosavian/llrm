extern unsigned hand_shrax7(unsigned x);
extern unsigned k_shrax7(unsigned x);

/* cc65 compiles a constant shift of a 16-bit value into a call of the routine under test,
   so the reference widens to long. */
static unsigned ref(unsigned x)
{
    return (unsigned)((unsigned long)x >> 7);
}

int main(void)
{
    unsigned i = 0;
    do {
        unsigned x = (unsigned)i;
        unsigned h = hand_shrax7(x);
        if (h != ref(x) || h != k_shrax7(x))
            return i < 255 ? i + 1 : 255;
    } while (++i);
    return 0;
}
