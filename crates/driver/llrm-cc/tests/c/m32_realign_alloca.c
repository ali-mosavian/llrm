void use(int *, int *);
/* an over-aligned local beside a dynamic alloca: needs the pin's base pointer (RA1) */
void f(int n) {
    _Alignas(64) int a[4];
    int *p = __builtin_alloca(n);
    use(a, p);
}
