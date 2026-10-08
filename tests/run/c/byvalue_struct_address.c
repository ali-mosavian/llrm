// flags: -O0 -m32 | -O2 -march=i486 -m32 -mabi=sysv | -Os -march=i486 -m32 -mabi=sysv
// The address of a by-value struct parameter, passed on: at -Os `lea eax, [ebp+12]` was left naming the frame register of a
// function that kept none (gcc.c-torture 20000706-2 aborted). Whether the frame register is kept is decided once.
extern void report(long value);

struct baz { int a, b, c, d, e; };

static int sum;

void bar(struct baz *x, int f, int g, int h, int i, int j)
{
    sum = x->a * 1 + x->b * 10 + x->c * 100 + x->d * 1000 + x->e * 10000 + f + g + h + i + j;
}

void foo(char *z, struct baz x, char *y)
{
    bar(&x, 6, 7, 8, 9, 10);
}

int main(void)
{
    struct baz x = { 1, 2, 3, 4, 5 };
    foo((char *)0, x, (char *)0);
    report(sum);
    return 0;
}
