int real(int x) { return x + 41; }
int other(int x) __attribute__((alias("real")));
int main(void) { return other(1) == 42 ? 0 : 1; }
