/* exit 0 when the read-modify-write functions of rmw_indexed.rs compute what C computes; else the index of the first wrong one */
void inc16(short *p, int i), add32(long *p, int i), sub8(char *p, int i), addneg16(short *p, int i), big16(short *p, int i),
    disp32(long *p), tally(const short *s, short *c, int n);
static long sys1(long n, long a) {
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1) : "memory");
    return d0;
}
int main(void) {
    short s[8] = { 3, 3, 250, 3, 17, 255, 0, 3 }, a[8] = { 0, 1, -1, 32767, 5, 6, 7, 8 }, c[256];
    for (int i = 0; i < 256; ++i) c[i] = 0;
    long l[8] = { 0, 1, -1, 2147483647, 5, 6, 7, 8 };
    char b[8] = { 0, 1, -1, 127, 5, 6, 7, 8 };
    short a0[8], big[8];
    long l0[8];
    char b0[8];
    for (int i = 0; i < 8; ++i) { a0[i] = a[i]; l0[i] = l[i]; b0[i] = b[i]; big[i] = a[i]; }
    for (int i = 0; i < 8; ++i) { inc16(a, i); add32(l, i); sub8(b, i); }
    for (int i = 0; i < 8; ++i) {
        if (a[i] != (short)(a0[i] + 1)) return 1;
        if (l[i] != (long)((unsigned long)l0[i] + 5)) return 2;
        if (b[i] != (char)(b0[i] - 2)) return 3;
    }
    for (int i = 0; i < 8; ++i) { addneg16(a, i); big16(big, i); }
    for (int i = 0; i < 8; ++i) {
        if (a[i] != (short)(a0[i] + 1 - 3)) return 4;
        if (big[i] != (short)(a0[i] + 1000)) return 5;
    }
    disp32(l);
    if (l[3] != (long)((unsigned long)l0[3] + 5 + 7)) return 6;
    tally(s, c, 8);
    if (c[3] != 4 || c[250] != 1 || c[17] != 1 || c[255] != 1 || c[0] != 1) return 7;
    return 0;
}
void _start(void) { sys1(1, main()); for (;;) ; }
