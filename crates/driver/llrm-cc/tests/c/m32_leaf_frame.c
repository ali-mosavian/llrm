/* one 4-byte local: the frame is 4 bytes (float_return reserved 8: the frame register's cell) */
int one(int *p) { volatile int t = *p; return t; }
int two(int *p) { volatile int t = *p; volatile int u = p[1]; return t + u; }
/* the x87 store of a narrowed double: one 4-byte cell (float_return `narrow`) */
float narrow(double x) { return (float)x; }
