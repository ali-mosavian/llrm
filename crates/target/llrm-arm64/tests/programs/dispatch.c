int printf(const char *, ...);
static int a(int x) { return x + 1; }
static int b(int x) { return x * 3; }
static int c(int x) { return x - 7; }
__attribute__((noinline)) static int run(int op, int x) {
  switch (op) {
    case 1: return a(x);
    case 2: return b(x);
    case 3: x += 5; /* fall through */
    case 4: return c(x);
    case 10: printf("ten\n"); return x;
    case 100: return a(b(x));
    case 1000: return c(c(x));
    case 70000: return x ^ 0xff;
    case -5: return -x;
    default: return 0;
  }
}
int main(void) {
  int ops[] = {1, 2, 3, 4, 10, 100, 1000, 70000, -5, 7, 0, -6};
  for (int i = 0; i < 12; i++) printf("%d ", run(ops[i], 20 + i));
  printf("\n");
  return 0;
}
