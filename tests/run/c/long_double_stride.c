// flags: -O0 -m32 | -O2 -m32
// A long double is 8 bytes with 4-byte alignment on the Watcom rows, as Open Watcom's libc reads it (a double): the array steps 8
// and the struct is 4 + 8 + 2 padded to 16. (gcc -m32's 12 is the sysv row's; the front end once made it 10, and an array stepped
// 10 where the code compiled for 12.)
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
