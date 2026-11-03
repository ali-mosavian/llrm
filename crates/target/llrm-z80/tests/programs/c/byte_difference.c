extern void report(long value);
static int diff(unsigned char a, unsigned char b) { return (int)a - (int)b; }
__attribute__((noinline)) int scmp(const char *a, const char *b)
{
    while (*a && *a == *b) { ++a; ++b; }
    return (unsigned char)*a - (unsigned char)*b;
}
int main(void)
{
    volatile unsigned char x = 200, y = 3, z = 255, w = 0;
    report(diff(x, y));
    report(diff(y, x));
    report(diff(z, w));
    report(diff(w, z));
    report(diff(x, x));
    report(scmp("abc", "abd"));
    report(scmp("abd", "abc"));
    report(scmp("abc", "abc"));
    report(scmp("", "\xff"));
    report(scmp("\xff", ""));
    return 0;
}
