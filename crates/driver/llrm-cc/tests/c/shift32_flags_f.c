/* A 64-bit shift by 32 wrote no flags to the effect table, so the two-address pass moved it between a `test` and its `jne`. */
extern unsigned long long get(int);
extern void op(void *, unsigned char, unsigned short);
extern void op2(void *, unsigned char);
extern void op3(void *, unsigned char, unsigned short);

void f(void *C, int x)
{
    const unsigned long long e = get(x);

    op(C, (e & 0xFFFF000000000000ull) == 0x0001000000000000ull ? 1 : e & (1ull << 50) ? 166 : 9, 1);

    const unsigned short t = e >> 32;

    if (e & (1ull << 50)) {
        op2(C, 166);
        op3(C, 5, t);
    } else
        op(C, 70, t);
}
