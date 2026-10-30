// flags: -Os -march=i486
/* An inlined store through a stack pointer, then calls writing the same object through a far pointer: -Os read the object as unchanged and returned 0 (#2982). */
extern void report(long value);

typedef struct { short state; } Trig;
typedef struct { short draw; } Brush;

static void frame(Brush far *b, short pressed)
{
    if (pressed)
        b->draw |= 2;
    else
        b->draw &= (short)~2;
}

static void ret(Trig far *t, Brush far *b)
{
    t->state = 3;
    frame(b, 0);
    frame(b, 0);
}

static short selftest(void)
{
    Trig t;
    Brush b;

    t.state = 0;
    b.draw = 1;
    frame(&b, -1);
    ret(&t, &b);
    return (short)(t.state == 3 && b.draw == 1);
}

int main(void)
{
    report(selftest());
    return 0;
}
