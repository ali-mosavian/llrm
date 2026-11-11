typedef unsigned long long u64;
static u64 b(double d) { union { double d; u64 u; } x = { d }; return x.u; }
static double d(u64 u) { union { double d; u64 u; } x; x.u = u; return x.d; }
double nul1(double a, double c) { return d(b(a) ^ 1); }
double nul2(int a) { return d((unsigned)a); }
double nul3(long long a) { return d((u64)a); }
