/* exit 0 when srl16..srl31, shl16..shl31 and sra16..sra31 (wide_shift.rs) compute what C computes; else 1 + the index of the first wrong one */
typedef unsigned (*fn)(unsigned);
#define ROW(p) p##16, p##17, p##18, p##19, p##20, p##21, p##22, p##23, p##24, p##25, p##26, p##27, p##28, p##29, p##30, p##31
#define DECL(p) unsigned p##16(unsigned), p##17(unsigned), p##18(unsigned), p##19(unsigned), p##20(unsigned), p##21(unsigned), p##22(unsigned), p##23(unsigned), p##24(unsigned), p##25(unsigned), p##26(unsigned), p##27(unsigned), p##28(unsigned), p##29(unsigned), p##30(unsigned), p##31(unsigned)
extern DECL(srl); extern DECL(shl); extern DECL(sra);
static long sys1(long n, long a) {
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1) : "memory");
    return d0;
}
int main(void) {
    static const fn srl[] = { ROW(srl) }, shl[] = { ROW(shl) }, sra[] = { ROW(sra) };
    static const unsigned v[] = { 0u, 1u, 0xFFFFFFFFu, 0x80000000u, 0x7FFFFFFFu, 0x12345678u, 0x80000001u, 0xFFFF0000u, 0x0000FFFFu, 0xA5C3F00Fu, 0x00008000u, 0x00010000u };
    for (unsigned k = 0; k < 16; ++k) {
        unsigned n = 16 + k;
        for (unsigned i = 0; i < sizeof v / sizeof v[0]; ++i) {
            unsigned x = v[i];
            if (srl[k](x) != x >> n) return 1 + k;
            if (shl[k](x) != x << n) return 17 + k;
            if (sra[k](x) != (unsigned)((int)x >> n)) return 33 + k;
        }
    }
    return 0;
}
void _start(void) { sys1(1, main()); for (;;) ; }
