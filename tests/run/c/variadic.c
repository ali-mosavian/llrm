// flags: -O2 -march=i486 | -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* A function with `...` reads its variable arguments from where they were pushed, on each target and ABI: the list pointed
   at a 16-bit address on -m32 and read garbage. A struct parameter before them is a copy of its words, not the arguments. */
typedef char *va_list;
#define va_start(ap, last) ((ap) = (char *)&(last) + ((sizeof(last) + sizeof(int) - 1) & ~(sizeof(int) - 1)))
#define va_arg(ap, type) (*(type *)(((ap) += sizeof(int)) - sizeof(int)))
#define va_end(ap) ((void)0)
extern void report(long value);

struct big { int i[6]; };

int sum(int n, ...)
{
    va_list ap;
    int total = 0;
    va_start(ap, n);
    while (n-- > 0)
        total += va_arg(ap, int);
    va_end(ap);
    return total;
}

int after_struct(struct big pa, int pb, ...)
{
    va_list ap;
    int last;
    va_start(ap, pb);
    last = va_arg(ap, int);
    va_end(ap);
    return pb + pa.i[5] * 10 + last * 100;
}

/* The address of a struct parameter steps into the arguments after it, as STDARG.H's va_start does. */
int walk(struct big pa, ...)
{
    char *p = (char *)&pa + sizeof pa;
    return *(int *)p + pa.i[0];
}

int main(void)
{
    struct big g;
    int i;
    for (i = 0; i < 6; i++)
        g.i[i] = i + 1;
    report(sum(4, 1, 2, 3, 4));
    report(sum(0));
    report(after_struct(g, 7, 5));
    report(walk(g, 40));
    return 0;
}
