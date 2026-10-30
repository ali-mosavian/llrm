// flags: -O2 -march=i486
// targets: x86-m16
/* A load through a huge pointer plus a constant whose offset wraps: the carry into the selector was computed and dropped
   (DAG selector), so `hp[10]` read big[4] for big[65540]. */
extern void report(long value);
unsigned char __huge big[140000];
unsigned char __huge *volatile hp;

int main(void)
{
    unsigned long i;
    volatile unsigned long start = 65530ul;
    for (i = 0; i < 140000; i++) big[i] = (unsigned char)(i % 251);
    hp = big + start;
    report(hp[10]);
    start = 131066ul;
    hp = big + start;
    report(hp[10]);
    return 0;
}
