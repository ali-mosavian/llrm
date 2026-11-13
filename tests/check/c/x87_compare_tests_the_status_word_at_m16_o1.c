// RUN: llrm-cc %s -m16 -O1 -march=i386 -S -o /dev/stdout
// DEVIATE X87-FCMP-AH at -O1 is m32's: the target keeps `sahf` where bytes are held, m16 does not (its counted cells
// are not judged; #9613 moved 12 of 31 float cells there, bytes -2..-6, clocks +0.04..+0.55%). m16 -O1 keeps `test ah`.
// CHECK-LABEL: f{{.*}} proc
// CHECK: fnstsw ax
// CHECK-NEXT: test ah, 65
// CHECK-NOT: sahf
double f(double a, double b) { return a > b ? a : b; }
