void *k_memmove(void *d, const void *s, unsigned n)
{
    unsigned char *dp = d;
    const unsigned char *sp = s;

    if (dp < sp) {
        while (n--)
            *dp++ = *sp++;
    } else {
        dp += n;
        sp += n;
        while (n--)
            *--dp = *--sp;
    }
    return d;
}
