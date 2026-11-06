// flags: -O1 | -O2 | -Os
// `(x & 0xFFFFFFFF00) == 0xCDEF123400` on a 5-byte field read as a dword at +1 shifted left by 8: the shift was narrowed to 32 bits because the source's upper half is zero, which loses byte 3 of the field (bitfield_long_long printed 0, not 1).
extern void report(long value);

struct s { unsigned long long a:8, b:32; };

__attribute__((noinline)) int same(struct s *p) { return p->b == 0xcdef1234UL; }

int main(void)
{
    static struct s i;
    i.a = 12;
    i.b = 0xcdef1234;
    report(same(&i));
    return 0;
}
