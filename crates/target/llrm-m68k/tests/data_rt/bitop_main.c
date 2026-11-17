/* exit 0 when set0..set31, clr0..clr31 and chg0..chg31 (bitop.rs) compute x | 1<<k, x & ~(1<<k) and x ^ 1<<k; else 1 + the index of the first wrong one */
typedef unsigned (*fn)(unsigned);
extern unsigned set0(unsigned), set1(unsigned), set2(unsigned), set3(unsigned), set4(unsigned), set5(unsigned), set6(unsigned), set7(unsigned), set8(unsigned), set9(unsigned), set10(unsigned), set11(unsigned), set12(unsigned), set13(unsigned), set14(unsigned), set15(unsigned), set16(unsigned), set17(unsigned), set18(unsigned), set19(unsigned), set20(unsigned), set21(unsigned), set22(unsigned), set23(unsigned), set24(unsigned), set25(unsigned), set26(unsigned), set27(unsigned), set28(unsigned), set29(unsigned), set30(unsigned), set31(unsigned);
extern unsigned clr0(unsigned), clr1(unsigned), clr2(unsigned), clr3(unsigned), clr4(unsigned), clr5(unsigned), clr6(unsigned), clr7(unsigned), clr8(unsigned), clr9(unsigned), clr10(unsigned), clr11(unsigned), clr12(unsigned), clr13(unsigned), clr14(unsigned), clr15(unsigned), clr16(unsigned), clr17(unsigned), clr18(unsigned), clr19(unsigned), clr20(unsigned), clr21(unsigned), clr22(unsigned), clr23(unsigned), clr24(unsigned), clr25(unsigned), clr26(unsigned), clr27(unsigned), clr28(unsigned), clr29(unsigned), clr30(unsigned), clr31(unsigned);
extern unsigned chg0(unsigned), chg1(unsigned), chg2(unsigned), chg3(unsigned), chg4(unsigned), chg5(unsigned), chg6(unsigned), chg7(unsigned), chg8(unsigned), chg9(unsigned), chg10(unsigned), chg11(unsigned), chg12(unsigned), chg13(unsigned), chg14(unsigned), chg15(unsigned), chg16(unsigned), chg17(unsigned), chg18(unsigned), chg19(unsigned), chg20(unsigned), chg21(unsigned), chg22(unsigned), chg23(unsigned), chg24(unsigned), chg25(unsigned), chg26(unsigned), chg27(unsigned), chg28(unsigned), chg29(unsigned), chg30(unsigned), chg31(unsigned);
static long sys1(long n, long a) {
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1) : "memory");
    return d0;
}
int main(void) {
    static const fn set[] = { set0, set1, set2, set3, set4, set5, set6, set7, set8, set9, set10, set11, set12, set13, set14, set15, set16, set17, set18, set19, set20, set21, set22, set23, set24, set25, set26, set27, set28, set29, set30, set31 }, clr[] = { clr0, clr1, clr2, clr3, clr4, clr5, clr6, clr7, clr8, clr9, clr10, clr11, clr12, clr13, clr14, clr15, clr16, clr17, clr18, clr19, clr20, clr21, clr22, clr23, clr24, clr25, clr26, clr27, clr28, clr29, clr30, clr31 }, chg[] = { chg0, chg1, chg2, chg3, chg4, chg5, chg6, chg7, chg8, chg9, chg10, chg11, chg12, chg13, chg14, chg15, chg16, chg17, chg18, chg19, chg20, chg21, chg22, chg23, chg24, chg25, chg26, chg27, chg28, chg29, chg30, chg31 };
    static const unsigned v[] = { 0u, 1u, 0xFFFFFFFFu, 0x80000000u, 0x7FFFFFFFu, 0x12345678u, 0xA5C3F00Fu, 0x00008000u };
    for (unsigned k = 0; k < 32; ++k)
        for (unsigned i = 0; i < sizeof v / sizeof v[0]; ++i) {
            unsigned x = v[i], b = 1u << k;
            if (set[k](x) != (x | b)) return 1 + k;
            if (clr[k](x) != (x & ~b)) return 33 + k;
            if (chg[k](x) != (x ^ b)) return 65 + k;
        }
    return 0;
}
void _start(void) { sys1(1, main()); for (;;) ; }
