/* recfloat -Os: the tail recursion becomes a loop whose rotation adds two `fxch`. */
static double decay(double x, double acc, int n)
{
    if (n == 0) return acc;
    return decay(x * 0.5 + 1.0, acc + x, n - 1);
}
double decay_all(int n) { return decay(100.0, 0.0, n); }
