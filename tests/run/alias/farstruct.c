/* A far pointer to a global, returned inside a struct by value: a store through it is the global's. */
extern void report(long value);

struct S {
    short far *p;
};

short g[2];

static struct S mk(void)
{
    struct S s;
    s.p = (short far *)&g[0];
    return s;
}

static void set(struct S *s)
{
    s->p[1] = 5;
}

int main(void)
{
    struct S s = mk();
    g[1] = 0;
    set(&s);
    report(g[1]);
    return 0;
}
