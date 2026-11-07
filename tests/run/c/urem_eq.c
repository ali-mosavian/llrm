// flags: -O2 -march=i486 -m32 | -O1 -march=i486 -m32
/* `x % d == c` and `!= c` for a constant d: the pin's mul/rotate/compare (buildUREMEqFold), no divide. */
extern void report(long value);

#define CELL(name, d, c, op) static int __attribute__((noinline)) name(unsigned int x) { return x % d op c; }
CELL(r5z, 5u, 0u, ==)
CELL(r12n3, 12u, 3u, !=)
CELL(r7e6, 7u, 6u, ==)
CELL(r10z, 10u, 0u, ==)
CELL(r3n2, 3u, 2u, !=)
CELL(r641z, 641u, 0u, ==)

int main(void)
{
    unsigned int x = 0, sum = 0, mix = 0;
    for (int i = 0; i < 4000; ++i) {
        sum += (unsigned)r5z(x) + 2 * r12n3(x) + 4 * r7e6(x) + 8 * r10z(x) + 16 * r3n2(x) + 32 * r641z(x);
        mix = mix * 31 + sum;
        x += 1234567u + (unsigned)i * 7u;
    }
    report(sum);
    report((long)(mix & 0x7fffffff));
    report(r5z(0xffffffffu) + 2 * r12n3(0xfffffffbu) + 4 * r7e6(0xffffffffu));
    return 0;
}
