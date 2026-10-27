// flags: -O2 -march=i486 -fno-inline-functions -fno-inline-functions-called-once | -O2 -march=i486 -m32 -fno-inline-functions -fno-inline-functions-called-once
/* Calls in tail position that reuse the caller's frame: the callee gets the caller's own arguments, a frame with saved
   registers and locals is taken down first, the callee's result is the caller's, and calls that cannot be sibling calls
   (swapped arguments, an extra argument, an escaping local) still return the right values. */
extern void report(long value);

long add3(long a, long b, long c) { return a + b * 2 + c * 3; }
long same(long a, long b, long c) { return add3(a, b, c); }
long extra(long a, long b) { return add3(a, b, 7); }
long swapped(long a, long b, long c) { return add3(c, b, a); }
long mixed(long a, long b, long c) { return add3(a, c, b) ^ 0x55; }

long volatile sink[4];

long framed(long a, long b, long c)
{
    long t[6], i;
    for (i = 0; i < 6; ++i) t[i] = a * i + b;
    sink[0] = t[3] + t[5];
    return add3(a, b, c);
}

long even(long n, long acc);
long odd(long n, long acc) { return n == 0 ? acc : even(n - 1, acc + 3); }
long even(long n, long acc) { return n == 0 ? acc + 1 : odd(n - 1, acc * 2 + 1); }

long countdown(long n, long acc) { if (n == 0) return acc; return countdown(n - 1, acc * 3 + n); }

long forty_two(void) { return 42; }
long (*volatile chosen)(void) = forty_two;
long through_pointer(void) { return chosen(); }

long volatile in[10] = { 1, 2, 3, 4, 5, 6, 7, 41, 40, 20 };

int main(void)
{
    report(same(in[0], in[1], in[2]));
    report(extra(in[3], in[4]));
    report(swapped(in[0], in[1], in[2]));
    report(mixed(in[1], in[2], in[3]));
    report(framed(in[4], in[5], in[6]));
    report(even(in[7], 0));
    report(odd(in[8], 1));
    report(countdown(in[9], 1) & 0xFFFFFF);
    report(through_pointer());
    return 0;
}
