#include <stdio.h>

/* r and m are live across calls to __multf3: the call's result is spilled. */
__attribute__((noinline)) long double power(long double a, unsigned n)
{
    long double r = 1, m = a;

    for (; n; n >>= 1) {
        if (n & 1)
            r *= m;

        m *= m;
    }

    return r;
}

int main(void)
{
    for (unsigned n = 0; n < 9; n++)
        printf("%d ", (int)power(2.0L, n));

    printf("\n");
    return 0;
}
