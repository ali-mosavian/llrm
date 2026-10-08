// flags: -O0 | -O2 | -Os
// A byval parameter is the callee's own copy: a function that writes it, called with a local or a global object, leaves the
// caller's object alone, however the optimizer treats the call (inlined, or its argument known).
extern void report(long value);

struct big { long a[10]; };
struct big shared;

static long peek(struct big b) { return b.a[3] + b.a[9]; }
static long bump(struct big b) { b.a[0] = 99; return b.a[0] + b.a[9]; }

int main(void)
{
    struct big g;
    int i;

    for (i = 0; i < 10; i++)
        g.a[i] = shared.a[i] = i + 1;
    report(bump(g));
    report(g.a[0]);
    report(bump(shared));
    report(shared.a[0]);
    report(peek(g));
    g.a[3] = 1000;
    report(peek(g));
    return 0;
}
