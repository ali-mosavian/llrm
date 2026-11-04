/* Calls the program's main (renamed orig_main) K times; report() sums instead of printing. */
#include <stdio.h>
#include <stdlib.h>
#include <mach/mach_time.h>
extern int orig_main(void);
static long sum, calls, first[8]; static int nfirst;
void report(long v) { sum += v; if (calls == 0 && nfirst < 8) first[nfirst++] = v; }
int main(int argc, char **argv) {
    int k = argc > 1 ? atoi(argv[1]) : 1, reps = argc > 2 ? atoi(argv[2]) : 15;
    mach_timebase_info_data_t tb; mach_timebase_info(&tb);
    if (argc > 3) { orig_main(); for (int i = 0; i < nfirst; i++) printf("%ld\n", first[i]); return 0; }
    for (int r = 0; r < reps; r++) {
        unsigned long long t0 = mach_absolute_time();
        for (int i = 0; i < k; i++) { orig_main(); calls++; }
        printf("%.4f\n", (double)(mach_absolute_time() - t0) * tb.numer / tb.denom / k / 1e3); /* us per run */
    }
    return sum == 42;
}
