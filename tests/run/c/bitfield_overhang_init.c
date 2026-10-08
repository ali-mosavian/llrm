// flags: -O0 -m32 | -O2 -m32
// A struct of a byte-sized and a 24-bit field, initialised with braces: Open Watcom stores the 24-bit field's int at
// offset 1, one byte past the struct, and cutting that back to the struct's end died with "Bad initializer quad"
// (11 gcc.c-torture programs: pr103417, bf64-1, bitfld-3, 20230630-1 ...).
extern void report(long value);

struct S { int a : 8; int b : 24; };
struct S g = { 0x5a, 0x123456 };
struct S table[2] = { { 1, 2 }, { 0x7f, -3 } };

int main(void)
{
    struct S l = { 3, 0x10203 };
    report(g.a);
    report(g.b);
    report(table[0].a);
    report(table[0].b);
    report(table[1].a);
    report(table[1].b);
    report(l.a);
    report(l.b);
    report(sizeof(struct S));
    return 0;
}
