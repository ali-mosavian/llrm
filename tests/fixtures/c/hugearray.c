/* A __huge array of 80000 bytes, past one 64K segment: each routine reads or
   writes it where an offset that wraps would give the wrong element. */
long __huge big[20000];

long hfill(void)
{
    long i;
    for (i = 0; i < 20000; i++) big[i] = i * 3 + 1;
    return big[19999];
}

long hsumidx(void)
{
    long s = 0, i;
    for (i = 0; i < 20000; i++) s += big[i];
    return s;
}

long hsumptr(void)
{
    long s = 0;
    long __huge *p = big;
    long __huge *e = big + 20000;
    while (p < e) { s += *p; p++; }
    return s;
}

long hdiff(void)
{
    long __huge *a = big + 19999;
    long __huge *b = big + 10;
    return a - b;
}

long hmid(void)
{
    long __huge *p = big;
    p += 17000;
    return *p;
}
