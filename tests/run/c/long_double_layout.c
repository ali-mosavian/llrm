// flags: -O0 | -O2 | -O0 -m32 | -O2 -m32
// sizeof, arrays and struct members of long double agree with each other and with __SIZEOF_LONG_DOUBLE__, the initialised ones
// holding their values (long_double_stride.c pins the 12 at m32).
extern void report(long value);

struct Mixed {
    char tag;
    long double value;
    short after;
};

long double table[3] = { 1.5L, 2.5L, 3.5L };
struct Mixed mixed = { 7, 4.25L, 9 };
long double single = 6.5L;
short last = 11;

int main(void)
{
    report((long)(sizeof(long double) == __SIZEOF_LONG_DOUBLE__));
    report((long)(sizeof(table) == 3 * sizeof(long double)));
    report((long)(sizeof(struct Mixed) >= 2 + sizeof(long double)));
    report((long)(table[0] * 2) + (long)(table[1] * 2) + (long)(table[2] * 2));
    report((long)(mixed.value * 4) + mixed.tag + mixed.after);
    report((long)(single * 2) + last);
    return 0;
}
