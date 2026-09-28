int __far f(int);
int __far g(long);

/* Reduced from qcport: BCC -Os cleans a call's arguments with pops. */
int caller(int a) { int x = f(a); return x + g((long)a); }
