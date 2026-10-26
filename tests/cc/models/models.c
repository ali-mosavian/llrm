char buf[4];
char *g(char *p) { char *q = buf; return p + (q - buf) + 1; }
int sum(int n, char *s) { int t = 0; while (n--) t += *s++; return t; }
int (*pick)(int, char *);
int call(int n, char *s) { pick = sum; return pick(n, s); }
