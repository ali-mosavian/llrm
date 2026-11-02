int puts(const char *);
static int a(int x) { return x + 1; }
static int b(int x) { return x * 3; }
static int c(int x) { return x - 7; }
__attribute__((noinline)) static int run(int op, int x) {
  switch (op) {
    case 1: return a(x);
    case 2: return b(x);
    case 3: x += 5; /* fall through */
    case 4: return c(x);
    case 100: return a(b(x));
    case 1000: return c(c(x));
    case 70000: return x ^ 0xff;
    case -5: return -x;
    default: return 0;
  }
}
int main(void) {
  int ops[] = {1, 2, 3, 4, 100, 1000, 70000, -5, 7, 0, -6};
  int sum = 0;
  for (int i = 0; i < 11; i++) sum = sum * 31 + run(ops[i], 20 + i);
  puts("done");
  return sum & 127;
}
