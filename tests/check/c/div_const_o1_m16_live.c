// RUN: llrm-cc %s -m16 -O1 -S -o /dev/stdout
// With more than four values live across a long remainder by a constant the
// `div` (which pins eax, edx and a divisor register) is the reciprocal
// multiply again at m16 -O1: huge's `(r * 201L + c) % 251` loop kept `div edi`
// and a spill, +9 B and +21% clk over the multiply.
unsigned long r(unsigned long *p)
{
    unsigned long a = p[0], b = p[1], c = p[2], d = p[3], e = p[4], f = p[5];
    unsigned long m = a % 251;
    return m + a + b + c + d + e + f;
}
// CHECK-LABEL: _r@
// CHECK-NOT: div
// CHECK: mul
