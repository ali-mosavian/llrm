// flags: -O0 | -O2 | -Os
// A product kept in a byte (gcc.c-torture arith-rand, pr44828): at -O2 the multiply is narrowed to an i8, which has no
// two-operand form, and the unit was refused with "Semantics(op=MULTIPLY, name='imul', dests=(Reg(register=1, width=1) ...".
extern void report(long value);

static unsigned char unsigned_product(unsigned char a, unsigned char b) { return a * b; }
static signed char signed_product(signed char a, signed char b) { return a * b; }
static unsigned char by_constant(unsigned char a) { return a * 200; }
static signed char by_negative(signed char a) { return a * -7; }

int main(void)
{
    long unsigned_sum = 0, signed_sum = 0, constants = 0;
    int a, b;
    for (a = 0; a < 256; a += 5)
        for (b = 0; b < 256; b += 7) {
            unsigned_sum += unsigned_product((unsigned char)a, (unsigned char)b);
            signed_sum += signed_product((signed char)a, (signed char)b);
        }
    for (a = 0; a < 256; ++a)
        constants += by_constant((unsigned char)a) * 3L + by_negative((signed char)a);
    report(unsigned_sum);
    report(signed_sum);
    report(constants);
    return 0;
}
