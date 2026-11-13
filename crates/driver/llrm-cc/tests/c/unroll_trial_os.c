extern void report(long value);

int main(void)
{
    int i;
    double oldrho = 0.0;
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
