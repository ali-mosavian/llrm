// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=pentium -O2 -S -o /dev/stdout
// DEVIATE X87-FCMP-AH is made where the table prices `sahf` above a register-immediate operation: the 486's.
// The Pentium table has no `sahf` clocks yet, so its compare keeps the pin's `sahf`.
// CHECK-LABEL: _f proc
// CHECK: fnstsw ax
// CHECK-NEXT: sahf
// CHECK-NOT: test ah
double f(double a, double b) { return a > b ? a : b; }
