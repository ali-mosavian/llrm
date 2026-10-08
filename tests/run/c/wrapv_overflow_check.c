// flags: -O2 -fwrapv | -Os -fwrapv
// With -fwrapv a signed add wraps, so `r < 0` after adding two non-negatives is a valid overflow check; without it C promises
// the sum fits and the check may go (gcc.c-torture 950704-1, which needs -fwrapv: dg-additional-options).
extern void report(long value);

int errflag;

long long f(long long x, long long y)
{
    long long r;

    errflag = 0;
    r = x + y;
    if (x >= 0) {
        if (y < 0 || r >= 0)
            return r;
    } else {
        if (y > 0 || r < 0)
            return r;
    }
    errflag = 1;
    return 0;
}

int g(long x, long y)
{
    long r = x + y;
    return x >= 0 && y >= 0 && r < 0;
}

int main(void)
{
    f(0x7fffffffffffffffLL, 1LL);
    report(errflag);
    f(0x8000000000000000LL, -1LL);
    report(errflag);
    f(1, -1);
    report(errflag);
    report(g(2147483647L, 1));
    report(g(1, 2));
    return 0;
}
