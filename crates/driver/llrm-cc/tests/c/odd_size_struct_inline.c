extern void report(long value);
struct five { char a, b, c, d, e; };

int fives(struct five f) { return f.a + f.e * 2; }

int main(void)
{
    struct five f;
    f.a = 1; f.b = 2; f.c = 3; f.d = 4; f.e = 5;
    report(fives(f));
    return 0;
}
