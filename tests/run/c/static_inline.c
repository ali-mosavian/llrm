// flags: -O0 | -O2 | -Os
// `static inline` functions, called (gcc.c-torture 20001130-1, 20050106-1: "undefined symbol bar_"): the front end marked them for
// expansion by a code generator that asks it to, which llrm's recording shim is not, so a call stayed a call to a function never
// emitted. They are functions like any other, and llrm's own inliner expands them on MIR.
extern void report(long value);

static inline int twice(int value) { return value + value; }
static __inline unsigned short low_half(unsigned long value) { return (unsigned short)value; }
static inline long fold(long total, int step) { return total * 3 + twice(step); }

typedef int (*step_function)(int);
static step_function chosen = twice;

int main(void)
{
    long total = 0;
    int i;
    for (i = 0; i < 6; ++i)
        total = fold(total, i);
    report(total);
    report(low_half(0x12345678UL));
    report(chosen(21));
    return 0;
}
