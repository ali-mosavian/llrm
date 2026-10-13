// flags: -O0 -m32 | -O2 -m32
// A long double is 12 bytes at m32 with 4-byte alignment, as gcc -m32 lays it out; Open Watcom's front end made it 10, and an
// array of them stepped 10 bytes where the code compiled for 12.
extern void report(long value);

struct Mixed {
    char tag;
    long double value;
    short after;
};

long double table[2] = { 1.5L, 2.5L };

int main(void)
{
    report((long)sizeof(long double));
    report((long)sizeof(table));
    report((long)sizeof(struct Mixed));
    report((long)((char *)&table[1] - (char *)&table[0]));
    return 0;
}
