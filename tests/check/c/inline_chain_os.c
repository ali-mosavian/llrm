// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// `foo` passes a byval struct on to `bar`, which sums it into a static; `main` calls `foo` with a constant struct.
// The pin inlines callee-first (bar into foo, cost 15 < 75; foo into main) and folds the sum. The -Os size guard
// priced each edge alone and kept both calls (byvalue_struct_address 147 B, clang 23). An SZ2 trial that also inlines
// the sites its splice brings sees `foo` and then `bar` fold in `main`.
// CHECK-LABEL: _main proc
// CHECK-NOT: call _foo
// CHECK-NOT: call _bar
// CHECK: call _report
// CHECK: _main endp
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
