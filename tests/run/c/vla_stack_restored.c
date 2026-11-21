// flags: -O0 | -O1 | -O2 | -Os
// targets: !m68k !z80-sdcc !6502-sim65 !65c02-sim65
/* A variable-length array in a loop is freed each trip (stacksave/stackrestore): every trip's array ends where the first one did. The save was taken after the allocation on wasm (-O1 and up), x86_64 -O1/-O2 and x86_32 -O1..-Os (a pass moved the save down past the allocation), so the stack pointer was restored to where the array already was and each trip took 16 bytes more. */
extern void report(long value);

static long first;

/* Where the stack stood before the array: its address plus its size. */
static long moved_from_first(long top, int trip)
{
    if (trip == 0) {
        first = top;
        return 0;
    }
    return top != first;
}

int main(void)
{
    volatile int step = 16;
    long moved = 0;
    int trip, i;

    for (trip = 0; trip < 6; trip++) {
        int n = step * (trip + 1);
        char a[n];

        for (i = 0; i < n; i++)
            a[i] = (char)(trip + i);
        moved += moved_from_first((long)a + n, trip);
        report(a[n - 1]);
    }
    report(moved);
    return 0;
}
