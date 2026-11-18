extern unsigned next(void);
extern void sink(void);

long f(const long *t)
{
    unsigned a = next() + 1, b = next() + 2, c = next() + 3, d = next() + 4, e = next() + 5, g = next() + 6,
             h = next() + 7, i = next() + 8;
    sink();
    return t[a] + t[b] + t[c] + t[d] + t[e] + t[g] + t[h] + t[i];
}
