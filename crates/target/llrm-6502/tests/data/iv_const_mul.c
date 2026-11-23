extern void report(long value);

static short s[48], t[49];
static signed char v[48];

int main(void)
{
    int i;
    for (i = 0; i < 48; ++i)
    {
        s[i] = (short)(i * 7 + 1);
        t[i] = (short)(i * 5 + 2);
        v[i] = (signed char)(i - 20);
    }
    report(s[47] + t[47] + v[47]);
    return 0;
}
