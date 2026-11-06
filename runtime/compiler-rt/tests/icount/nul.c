typedef union { unsigned long long u; double d; } U;
double nul1(double a, double b) { return a; }
double nul2(int a) { U x; x.u = (unsigned)a; return x.d; }
double nul3(long long a) { U x; x.u = (unsigned long long)a; return x.d; }
int nul4(double a) { U x; x.d = a; return (int)x.u; }
float nul5(float a, float b) { return a; }
int nul6(double a, double b) { U x; x.d = a; return (int)x.u; }
