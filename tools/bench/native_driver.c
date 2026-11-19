/* Runs the program's kernel (`main` renamed bench_main_) N times and prints what `report` was given once. No clock and no sort: a driver whose own
   instruction count depended on the timings it took made two runs of one binary differ by 13 of 500 instructions. */
#include <stdio.h>
#include <stdlib.h>

extern int bench_main_(void);
static long last;
static int got;

void report(long v)
{
    if (!got) {
        last = v;
        got = 1;
    }
}

int main(int argc, char **argv)
{
    int reps = argc > 1 ? atoi(argv[1]) : 1;
    for (int i = 0; i < reps; i++)
        bench_main_();
    printf("%d\n", (int)last);
    return 0;
}
