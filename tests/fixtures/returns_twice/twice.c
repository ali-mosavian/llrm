/* setjmp returns twice. What the second return must find: a local nothing
 * changed after setjmp, one held for the whole run, and a volatile one. */
#include <setjmp.h>
#include <stdio.h>

static jmp_buf env;
static int thrown;

static int seed(int k)
{
    return k * 7 + 3;
}

static void thrower(int k)
{
    thrown++;
    longjmp(env, k);
}

int main(void)
{
    int a = seed(1);
    int b = seed(2);
    int c = seed(3);
    int d = seed(4);
    volatile int changed = 1;
    int got = setjmp(env);

    if (got == 0) {
        changed = 99;
        thrower(5);
    }
    printf("got=%d a=%d b=%d c=%d d=%d changed=%d thrown=%d\n", got, a, b, c, d, changed, thrown);
    return 0;
}
