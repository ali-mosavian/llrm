/* QB PMAP: two inlined arms of one Dekker double-double computation, each reading its own offset (wymin or wymax). Expected: "199 0". */
#include <stdio.h>
static int scaled_round(double v, double offset, double scale)
{
    static const double split = 134217729.0;
    double d, bb, derr, p, t, ah, al, bh, bl, lo, part;
    long whole;
    d = v - offset; bb = d - v;
    derr = (v - (d - bb)) + (-offset - bb);
    p = d * scale; t = split * d; ah = t - (t - d); al = d - ah;
    t = split * scale; bh = t - (t - scale); bl = scale - bh;
    lo = ((ah * bh - p) + ah * bl + al * bh) + al * bl + derr * scale;
    whole = (long)p; part = (p - whole) + lo;
    if (part >= 0.5) whole += part == 0.5 && !(whole & 1) ? 0 : 1;
    else if (part <= -0.5) whole -= part == -0.5 && !(whole & 1) ? 0 : 1;
    return (int)whole;
}
double wymin = 0, wymax = 199, lydif = 1.0;
int window_screen;
__attribute__((noinline)) float f(float expr)
{ return (float)(window_screen ? scaled_round(expr, wymin, lydif) : -scaled_round(expr, wymax, lydif)); }
int main(void){ window_screen = 0; printf("%g ", f(0)); window_screen = 1; printf("%g\n", f(0)); return 0; }
