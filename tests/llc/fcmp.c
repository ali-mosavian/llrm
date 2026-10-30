#include <stdio.h>
volatile double va = 1.5, vb = 2.5, vnan = 0.0;
volatile float fa = 1.5f, fb = 2.5f;
int main(void) {
    double a = va, b = vb, n = vnan / vnan;
    float x = fa, y = fb, m = (float)n;
    int r = 0;
    r |= (a < b) << 0; r |= (a <= b) << 1; r |= (a > b) << 2; r |= (a >= b) << 3;
    r |= (a == b) << 4; r |= (a != b) << 5;
    r |= (n < b) << 6; r |= (n == n) << 7; r |= (n != n) << 8; r |= (b > n) << 9;
    r |= (x < y) << 10; r |= (x >= y) << 11; r |= (m == m) << 12; r |= (m != x) << 13;
    printf("%d\n", r);
    printf("%d %d\n", a < b ? 10 : 20, n < b ? 10 : 20);
    return 0;
}
