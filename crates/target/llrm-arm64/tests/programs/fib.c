int printf(const char *, ...);
int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
int main(void) {
  for (int i = 0; i < 15; i++) printf("%d ", fib(i));
  printf("\n");
  return fib(10) - 55;
}
