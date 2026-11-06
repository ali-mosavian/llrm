// flags: -Os | -O2 | -Os -m32 | -O2 -m32 | -Os -m32 -mabi=sysv
/* A local's address in a function of six arguments, two of them on the stack: after `mov ebp, ebx` the frame base was renamed with
   the copy, and `lea eax, [bx-52]` gave the callee the address of an argument less 52 (-m32 -Os). */
extern void report(long value);

typedef struct L { void *dst; unsigned count; int d, m4, s4, steps[2]; unsigned phase, xm, bit, px; const unsigned char *tab; unsigned color; } L;

static unsigned char cells[64];
static unsigned pitch = 320;
static long seen;

void terms(L *l, unsigned dx, unsigned dy)
{
    l->count = dx + dy;
    l->d = (int)dx - (int)dy;
    l->m4 = 4;
    seen = (long)dx * 100 + dy;
}

unsigned char *at(unsigned x, unsigned y)
{
    return cells + (x + y) % 64;
}

void kernel(L *l)
{
    report((long)(l->count) * 1000 + l->steps[0] + l->steps[1] + l->phase + (l->dst == at(1, 2)) + seen);
}

void line(const void *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y)
{
    L l;

    (void)fill;
    terms(&l, dx, dy);
    l.dst = at(x, y);
    l.steps[0] = l.steps[1] = step_y > 0 ? (int)pitch : -(int)pitch;
    l.phase = 0;
    kernel(&l);
}

int main(void)
{
    line(0, 1, 2, 7, 5, 1);
    line(0, 3, 4, 9, 2, -1);
    return 0;
}
