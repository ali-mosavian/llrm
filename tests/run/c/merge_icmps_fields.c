// flags: -O2 -march=i486
// targets: x86-m32 x86-m64
/* A chain of equal-offset field compares through parameters known dereferenceable: the pin's MergeICmps makes it one memcmp, so the
   pass on and off differ in bytes (pin m32 -O2: 147 B on, 150 B off). The result is a _Bool: the phi MergeICmps looks for is i1, an int result is not it. */
extern void report(long value);

struct S { int a, b, c; };

static _Bool __attribute__((noinline)) same(const struct S p[static 1], const struct S q[static 1])
{
    return p->a == q->a && p->b == q->b && p->c == q->c;
}

int main(void)
{
    struct S x = {1, 2, 3}, y = {1, 2, 3}, z = {1, 2, 4}, w = {0, 2, 3};
    report(same(&x, &y));
    report(same(&x, &z));
    report(same(&x, &w));
    return 0;
}
