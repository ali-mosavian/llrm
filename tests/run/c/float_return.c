// flags: -O0 | -O2 -march=i486 | -O0 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* A double and a float returned from computed operands, under --isel=dag. */
extern void report(long v);
double scale(double x, double y) { return y * 2.0 + x; }
float narrow(double x) { return (float)x; }
int main(void) {
    report((long)scale(1.0, 20.0));
    report((long)narrow(7.0));
    return 0;
}
