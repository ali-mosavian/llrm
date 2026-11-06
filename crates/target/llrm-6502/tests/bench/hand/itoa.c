char *k_itoa(int value, char *buf, int radix)
{
    char tmp[16];
    char *p = buf;
    unsigned u = (unsigned)value;
    unsigned char n = 0, d;

    if (radix == 10 && value < 0) {
        *p++ = '-';
        u = 0u - u;
    }
    do {
        d = (unsigned char)(u % radix) + '0';
        if (d > '9')
            d += 'A' - '9' - 1;
        tmp[n++] = d;
        u /= radix;
    } while (u);
    while (n)
        *p++ = tmp[--n];
    *p = 0;
    return buf;
}
