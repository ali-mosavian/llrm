typedef struct { int *sp; int *loc; } Regs;
enum { SRC_TOP, SRC_LOC };
static inline int operand(Regs *r, int src) { return src == SRC_LOC ? r->loc[0] : r->sp[-1]; }
static inline int step(Regs *r, int src, int op)
{
    const int b = operand(r, src);
    int *top = r->sp;
    int v = 0, bad = 0;
    switch (op) {
    case 0: bad = __builtin_add_overflow(*top, b, &v); break;
    case 1: bad = __builtin_sub_overflow(*top, b, &v); break;
    case 2: bad = __builtin_mul_overflow(*top, b, &v); break;
    case 3: v = *top & b; break;
    case 4: v = *top | b; break;
    default: v = *top ^ b; break;
    }
    if (bad)
        return 1;
    *top = v;
    return 0;
}
int add_t(Regs *r) { return step(r, SRC_TOP, 0); }
int sub_t(Regs *r) { return step(r, SRC_TOP, 1); }
int mul_t(Regs *r) { return step(r, SRC_TOP, 2); }
int and_t(Regs *r) { return step(r, SRC_TOP, 3); }
int add_l(Regs *r) { return step(r, SRC_LOC, 0); }
int sub_l(Regs *r) { return step(r, SRC_LOC, 1); }
int mul_l(Regs *r) { return step(r, SRC_LOC, 2); }
int or_l(Regs *r) { return step(r, SRC_LOC, 4); }
