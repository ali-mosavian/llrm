// flags: -O2
// A loop-carried double updated in place (`work /= 2.0`) that nothing reads after the loop: the allocator saw the update define a
// value already on the x87 stack and refused it, "floating stack result is not a fresh value" (gcc.c-torture pr29798 at -O2, from
// the day -O2 switched tree-sra on).
extern void report(long value);

int main(void)
{
    int i;
    double oldrho;
    double beta = 0.0;
    double work = 1.0;

    for (i = 1; i <= 2; i++) {
        double rho = work * work;
        if (i != 1)
            beta = rho / oldrho;
        if (beta == 1.0)
            report(-1);
        work /= 2.0;
        oldrho = rho;
    }
    report(7);
    return 0;
}
