/* exit 0 when every form of data_rt/bswap_forms.ll computes what C computes; else the bit of the first wrong one */
extern unsigned bs(unsigned), bs_low(unsigned), bs_high(unsigned), rev(unsigned), rev_loop(unsigned);
static unsigned c_rev(unsigned x) { unsigned r = 0; for (int i = 0; i < 32; ++i) r |= ((x >> i) & 1u) << (31 - i); return r; }
static long sys1(long n, long a) {
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1) : "memory");
    return d0;
}
int main(void) {
    static const unsigned v[] = { 0x11223344u, 0u, 1u, 0xFFFFFFFFu, 0x12345678u, 0x80000000u, 0xA5C3F00Fu };
    int bad = 0;
    for (unsigned i = 0; i < sizeof v / sizeof v[0]; ++i) {
        unsigned x = v[i], sw = (x << 24) | ((x & 0xFF00u) << 8) | ((x >> 8) & 0xFF00u) | (x >> 24);
        if (bs(x) != sw) bad |= 1;
        if (bs_low(x) != (sw & 255)) bad |= 2;
        if (bs_high(x) != (sw & 0xFF000000u)) bad |= 4;
        if (rev(x) != c_rev(x)) bad |= 8;
    }
    unsigned s = 2463534242u, sum = 0;
    for (int i = 0; i < 50; ++i) { s ^= s << 13; s ^= s >> 17; s ^= s << 5; sum += c_rev(s) & 255; }
    if (rev_loop(50) != sum) bad |= 16;
    return bad;
}
void _start(void) { sys1(1, main()); for (;;) ; }
