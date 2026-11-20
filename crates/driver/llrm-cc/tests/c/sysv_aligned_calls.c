void g(int, int);
void h(void);
int f(int a) {
    g(a, 1);
    h();
    return a;
}
