void report(long v)
{
    char buf[16];
    unsigned long u = v < 0 ? 0UL - (unsigned long)v : (unsigned long)v;
    static const unsigned long pow[] = {1000000000UL, 100000000UL, 10000000UL, 1000000UL, 100000UL, 10000UL, 1000UL, 100UL, 10UL, 1UL};
    int n = 0, started = 0, i;
    if (v < 0) buf[n++] = '-';
    for (i = 0; i < 10; i++) {
        char d = '0';
        while (u >= pow[i]) { u -= pow[i]; d++; }
        if (d != '0' || started || i == 9) { buf[n++] = d; started = 1; }
    }
    buf[n++] = '\n';
    register long d0 __asm__("d0") = 4;
    register long d1 __asm__("d1") = 1;
    register char *d2 __asm__("d2") = buf;
    register long d3 __asm__("d3") = n;
    __asm__ volatile("trap #0" : "+r"(d0) : "r"(d1), "r"(d2), "r"(d3) : "memory");
}
__asm__(".globl _start\n_start:\n\tjsr main\n\tmove.l %d0,%d1\n\tmoveq #1,%d0\n\ttrap #0\n");
