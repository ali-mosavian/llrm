int printf(const char *, ...);
static int dense(int x) {
  switch (x) {
    case 0: return 10; case 1: return 11; case 2: return 25; case 3: return 31;
    case 4: return 47; case 5: return 59; case 6: return 61; case 7: return 77;
    default: return -1;
  }
}
static int sparse(int x) {
  switch (x) {
    case 3: return 1; case 100: return 2; case 5000: return 3; case -7: return 4;
    default: return 0;
  }
}
static const char *name(int x) {
  switch (x) { case 0: return "zero"; case 1: return "one"; case 2: return "two"; default: return "many"; }
}
int main(void) {
  for (int i = -2; i < 10; i++) printf("%d:%d %s\n", i, dense(i), name(i));
  int xs[] = {3, 100, 5000, -7, 4};
  for (int i = 0; i < 5; i++) printf("%d ", sparse(xs[i]));
  printf("\n");
  return 0;
}
