/* A proven direct infinite loop makes its caller's following return unreachable. */
static void spinForever(void)
{
    volatile short tick = 0;
    while (1) {
        tick = (short)(tick + 1);
    }
}

short entersSpin(void)
{
    spinForever();
    return 7;
}
