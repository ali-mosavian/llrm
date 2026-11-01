struct S { int a, b, c, d, e; char f; };
int k(struct S *d, const struct S *s) { *d = *s; return d->c + d->f; }
