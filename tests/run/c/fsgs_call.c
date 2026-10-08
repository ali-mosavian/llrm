// flags: -O2 -march=i486 -mabi=watcom
// link: fsgs_call_callee.c
/* A far pointer kept across calls of a routine in another unit. The routine loads two more far pointers, using FS as well as ES; the
   caller read through its own pointer after each call, and under -mabi=watcom (and any convention with registers) it still held it in FS. */
extern void report(long value);
typedef struct { short far *a; short far *b; } Pair;
extern void store_pair(Pair far *p, int i, int x, int y);

#define MK_FP(seg, off) ((void far *)(((unsigned long)(seg) << 16) | (off)))

long run(Pair far *q)
{
    long sum = 0;
    int i;

    for (i = 0; i < 4; i++) {
        store_pair(q, i, i + 1, i * 10);
        sum += q->a[i] + q->b[i];
    }
    return sum;
}

int main(void)
{
    Pair p;
    int i;

    /* Two arrays in segments of their own: the routine's FS is then not the caller's. */
    p.a = (short far *)MK_FP(0x5000, 0);
    p.b = (short far *)MK_FP(0x5100, 0);
    for (i = 0; i < 4; i++) p.a[i] = p.b[i] = 0;
    report(run((Pair far *)&p));
    return 0;
}
