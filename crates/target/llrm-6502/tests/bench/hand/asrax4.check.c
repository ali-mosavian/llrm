extern int hand_asrax4(int x);
extern int k_asrax4(int x);

/* cc65 compiles a constant shift of a 16-bit value into a call of the routine under test,
   so the reference widens to long. */
static int ref(int x)
{
    return (int)((long)x >> 4);
}

int main(void)
{
    unsigned i = 0;
    do {
        int x = (int)i;
        int h = hand_asrax4(x);
        if (h != ref(x) || h != k_asrax4(x))
            return i < 255 ? i + 1 : 255;
    } while (++i);
    return 0;
}
