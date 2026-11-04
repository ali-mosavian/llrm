/* Runs bench_main_ `reps` times and prints its first result. No timing, no
   sort, no formatted timings: the instruction count of a run depends only on
   the code under test, so two runs of one binary count the same. */
#include <stdio.h>
#include <stdlib.h>
extern int bench_main_(void);
static long last;
static int got;
void report(long v) { if (!got) { last = v; got = 1; } }
int main(int argc, char **argv) {
    int reps = argc > 1 ? atoi(argv[1]) : 1;
    for (int i = 0; i < reps; i++) bench_main_();
    printf("%ld\n", last);
    return 0;
}
