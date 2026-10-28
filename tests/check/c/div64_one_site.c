// RUN: llrm-cc %s -m16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// A program with one i64 divide called the shared `$div64_s32` procedure: its call, frame and return cost more than the body pasted at the one site.
long long one_site(long long a) { return a / 1000000; }
// CHECK-LABEL: _one_site@3 proc far
// CHECK-NOT: div64
