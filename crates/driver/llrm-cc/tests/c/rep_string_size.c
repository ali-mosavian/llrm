typedef struct { unsigned a, b, c, d, e, f, g; } Regs;
typedef struct { unsigned a, b, c; } Tri;
extern void sink(Regs *);

void clear(void)
{
    Regs r = {0};
    sink(&r);
}

void copy3(Tri *to, const Tri *from) { *to = *from; }
