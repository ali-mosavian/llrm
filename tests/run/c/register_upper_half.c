// flags: -O2 -fno-inline-functions -mabi=watcom | -O2 -fno-inline-functions -m32 -mabi=watcom
// targets: !x86-m64
/* A callee that keeps CX and SI by pushing their 16-bit halves leaves the upper halves of ECX and ESI as it used them: a caller holding a
   long in ECX across a call read -65527 (the high word -1) for 9. */
extern void report(long value);
static long descend(long n, long *total)
{
    long cell = n * 2;

    if (n == 0) return *total;
    *total += cell;
    return n - descend(n - 1, total) + cell;
}
/* The depths come from volatile objects: gcc evaluated both recursions at compile time, so a clock census measured no kernel on its side. */
static volatile long depth_a = 2, depth_b = 10;
int main(void)
{
    long total = 0;
    report(descend(depth_a, &total));
    report(total);
    report(descend(depth_b, &total));
    report(total);
    return 0;
}
