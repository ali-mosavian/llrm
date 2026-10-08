// RUN: llrm-c %s -m32 {-O2 | -Os} -march=i486 -fno-inline-functions -S -o /dev/stdout
// `push ebp; mov ebp,esp ... leave` was three instructions a call, and ebp held no value (bench/fib).
// A function whose cells the stack pointer can name keeps no frame register.
// CHECK-LABEL: args_ proc
// CHECK-NOT: ebp
// CHECK: args_ endp
// CHECK-LABEL: locals_ proc
// CHECK-NOT: ebp
// CHECK: locals_ endp
// CHECK-LABEL: recur_ proc
// CHECK-NOT: ebp
// CHECK: recur_ endp
extern long sink(long *p, long n);
long args(long a, long b, long c, long d, long e) { return sink(0, a) - b + c * d + e; }
long locals(long n) { long t[6]; t[0] = n; t[5] = n + 1; return sink(t, 6) + t[0] + t[5]; }
long recur(long n) { return n < 2 ? n : n - recur(n - 1) + recur(n - 2) * 3; }
