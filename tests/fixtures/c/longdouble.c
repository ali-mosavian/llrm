/* long double conversions (#103): each was E1090 in the front end, and an
   operand of mixed arithmetic was left unconverted. */
extern void report(long v);

long double g = 3;
long double table[3];

double f(long double x) { return (double)x; }
long double h(double x) { return (long double)x; }
long k(long double x) { return (long)x; }

void main(void)
{
    long double a = h(2.5);
    long double b = a * 4 + 1;
    table[1] = b;
    table[2] = h(0.5) + table[1];
    report(k(b));
    report(k(table[2] * 2));
    report((long)f(table[2] * 1000));
    report(k(g / 2 * 10));
    report(k(-a * 3));
    report((long)(float)b);
    report(table[2] > table[1]);
    /* a constant folds in double, not through float: 0.1f * 1e9 is 100000001 */
    report((long)((double)(long double)0.1 * 1e9));
    report((long)((double)(0.1L + 0.0L) * 1e9));
}
