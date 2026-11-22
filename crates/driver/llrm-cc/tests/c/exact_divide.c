struct T { char a[48]; };

long diff(struct T *b, struct T *e) { return e - b; }
unsigned count(struct T *b, struct T *e) { return (unsigned)(e - b); }
long negated(long x) { return __builtin_expect(x % 48 == 0, 1) ? x / -48 : 0; }
