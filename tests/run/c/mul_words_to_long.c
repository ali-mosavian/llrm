// flags: -O0 | -O2 | -Os | -O2 -m32 | -Os -m32
/* The product of two zero-extended words is one 16 x 16 -> 32 multiply: the edge values times each other, and a word times
   itself. */
extern void report(long value);

unsigned long product(unsigned short a, unsigned short b) { return (unsigned long)a * b; }
unsigned long square(unsigned short a) { return (unsigned long)a * a; }

int main(void)
{
    static const unsigned short values[] = {0, 1, 2, 255, 256, 12345, 32767, 32768, 54321, 65535};
    for (int i = 0; i < 10; i++) {
        report((long)square(values[i]));
        for (int j = 0; j < 10; j++)
            report((long)product(values[i], values[j]));
    }
    return 0;
}
