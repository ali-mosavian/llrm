/* A loop to the terminator: the index is the post-incremented one, started at -1. */
unsigned len(const char *s)
{
    unsigned n = 0;
    while (s[n]) ++n;
    return n;
}
