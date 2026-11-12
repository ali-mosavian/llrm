// RUN: llrm-cc %s -m64 -O2 -S -o /dev/stdout
// `x / -3` and `x % -3` multiply once (DEVIATE SD-DIVMAG, the divide by the magnitude negated): the quotient used
// the negative divisor's own magic and the remainder the positive one, two `imul`. div_constants_long -O2 -199 B.
// CHECK-LABEL: qr proc
// CHECK: imul
// CHECK-NOT: imul
void qr(int x, int *o) { o[0] = x / -3; o[1] = x % -3; }
