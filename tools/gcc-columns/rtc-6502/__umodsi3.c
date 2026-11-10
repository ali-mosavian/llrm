extern unsigned long __udivmodsi4 (unsigned long, unsigned long, unsigned long *);
unsigned long __umodsi3 (unsigned long n, unsigned long d) { unsigned long r; __udivmodsi4 (n, d, &r); return r; }
