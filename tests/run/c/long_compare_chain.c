// flags: -O0 | -O2 | -Os
// The order of two longs on a 16-bit machine, a value and a branch, signed and unsigned, against a variable and a constant:
// each row of the table is compared with every other, and the bit of each answer is reported.
extern void report(long value);

#define OP(name, type, op) \
    static int __attribute__((noinline)) name(type a, type b) { return a op b; } \
    static int __attribute__((noinline)) name##_if(type a, type b) { if (a op b) return 1; return 0; }
OP(slt, long, <)
OP(sge, long, >=)
OP(sle, long, <=)
OP(sgt, long, >)
OP(ult, unsigned long, <)
OP(uge, unsigned long, >=)
OP(ule, unsigned long, <=)
OP(ugt, unsigned long, >)

static int __attribute__((noinline)) slt_const(long a) { return a < 70000L; }
static int __attribute__((noinline)) sge_const(long a) { return a >= -70000L; }
static int __attribute__((noinline)) ult_const(unsigned long a) { return a < 0x80010000UL; }
static int __attribute__((noinline)) uge_const(unsigned long a) { return a >= 65536UL; }

static const long v[] = {0L, 1L, -1L, 0x7FFFL, 0x8000L, 0xFFFFL, 0x10000L, -0x10000L, 0x7FFFFFFFL, -0x7FFFFFFFL - 1,
                         0x00010001L, 0xFFFF0000L, -70000L, 70000L, 0x80010000L};
#define N (sizeof v / sizeof v[0])

int main(void)
{
    unsigned i, j;
    long m[8], c[4];

    for (i = 0; i < N; ++i) {
        for (j = 0; j < 8; ++j)
            m[j] = 0;
        for (j = 0; j < 4; ++j)
            c[j] = 0;
        for (j = 0; j < N; ++j) {
            long a = v[i], b = v[j];
            unsigned long x = a, y = b;
            m[0] |= (long)(slt(a, b) | slt_if(a, b) << 1) << (2 * j);
            m[1] |= (long)(sge(a, b) | sge_if(a, b) << 1) << (2 * j);
            m[2] |= (long)(sle(a, b) | sle_if(a, b) << 1) << (2 * j);
            m[3] |= (long)(sgt(a, b) | sgt_if(a, b) << 1) << (2 * j);
            m[4] |= (long)(ult(x, y) | ult_if(x, y) << 1) << (2 * j);
            m[5] |= (long)(uge(x, y) | uge_if(x, y) << 1) << (2 * j);
            m[6] |= (long)(ule(x, y) | ule_if(x, y) << 1) << (2 * j);
            m[7] |= (long)(ugt(x, y) | ugt_if(x, y) << 1) << (2 * j);
        }
        c[0] = slt_const(v[i]);
        c[1] = sge_const(v[i]);
        c[2] = ult_const(v[i]);
        c[3] = uge_const(v[i]);
        for (j = 0; j < 8; ++j)
            report(m[j]);
        report(c[0] | c[1] << 1 | c[2] << 2 | c[3] << 3);
    }
    return 0;
}
