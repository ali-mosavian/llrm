// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
/* A __huge array of 80000 bytes, past one 64K segment, written and summed by index and by pointer,
   with a pointer difference and a step: where code32 is flat, the output is code16's. */
extern void report(long value);
long __huge big[20000];

int main(void)
{
    long s = 0, t = 0, i;
    long __huge *p = big;
    long __huge *e = big + 20000;
    for (i = 0; i < 20000; i++) big[i] = i * 3 + 1;
    for (i = 0; i < 20000; i++) s += big[i];
    while (p < e) { t += *p; p++; }
    report(big[19999]);
    report(s);
    report(t);
    report((big + 19999) - (big + 10));
    p = big;
    p += 17000;
    report(*p);
    return 0;
}
