// flags: -Os -march=i486 -m32
/* A quotient loop at -Os: the dividend of idiv is copied into eax each trip; a split copy of it was given the class of its own
   instructions (any register), placed in ebp, and the copy to eax was gone: recgcd printed 6284 for 3454. */
extern void report(long value);

static int gcd(int a, int b)
{
    if (b == 0) return a;
    return gcd(b, a % b);
}

long total(int n)
{
    long sum = 0;
    int i;

    for (i = 1; i <= n; ++i) sum += gcd(i * 7 + 3, i * 5 + 2) + gcd(i + 90, 360);
    return sum;
}

int main(void)
{
    report(total(300));
    return 0;
}
