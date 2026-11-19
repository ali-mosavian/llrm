extern int g(int);
int f(int x) {
    int a = g(x);
    int b = g(a);
    return a + b;
}
