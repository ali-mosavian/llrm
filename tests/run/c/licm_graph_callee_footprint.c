// flags: -O2 -fno-inline-functions | -Os -fno-inline-functions -m32
/* A call that touches only its own global does not clobber the loop's sum: the memory graph LICM keeps (held, with each call's
   footprint) answered a load of the sum with the loop phi where a graph made with calls unresolved said the call, and
   LLRM_CHECK_LICM_GRAPH compared the two (the identity gate's red on div_constants_*, 29 programs). */
extern void report(long value);

static unsigned long state = 5;
static unsigned long next(void) { state = state * 3UL + 1UL; return state; }

static unsigned long sum;

void run(void)
{
    long i;

    for (i = 0; i < 1500; ++i) {
        unsigned long w = next();

        sum += w;
    }
}

int main(void)
{
    run();
    report((long)(sum & 0x7FFFFFFFUL));
    return 0;
}
