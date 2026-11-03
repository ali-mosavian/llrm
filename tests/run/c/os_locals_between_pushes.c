// flags: -Os -march=i486 -m32 -mabi=sysv | -Os -march=i486 -m32
/* Locals addressed through esp once the frame register is omitted at -Os: eight volatile locals are read as eight pushed arguments, so
   each read happens with the earlier ones already on the stack. A wrong depth reads another cell. */
extern void report(long value);

static long __attribute__((noinline)) pack(long a, long b, long c, long d, long e, long f, long g, long h)
{
    return (((((a * 10 + b) * 10 + c) * 10 + d) * 10 + e) * 10 + f) * 100 + g * 10 + h;
}

static long __attribute__((noinline)) probe(long k)
{
    long loc[8];
    volatile long *p = loc;
    int i;
    for (i = 0; i < 8; i++) loc[i] = (k + i) % 10;
    return pack(p[7], p[6], p[5], p[4], p[3], p[2], p[1], p[0]);
}

int main(void)
{
    report(probe(1));
    report(probe(2));
    return 0;
}
