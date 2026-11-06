// flags: -O2 | -O2 -march=i486 -m32 -mabi=sysv
// A byval parameter is the callee's own copy: the callee writing its first bytes does not initialize the caller's object, so the caller's earlier
// store to it is live (an `initializes` range on a byval parameter had call effects delete `g.a[0] = 1`: byval_inline, #P1).
extern void report(long value);

struct big { long a[10]; };

static long bump(struct big b) { b.a[0] = 99; return b.a[0]; }

int main(void)
{
    struct big g;
    g.a[0] = 1;
    report(bump(g));
    report(g.a[0]);
    return 0;
}
