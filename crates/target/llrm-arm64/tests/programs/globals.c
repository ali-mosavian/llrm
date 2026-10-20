int printf(const char *, ...);
int table[8] = {3, 1, 4, 1, 5, 9, 2, 6};
const char *names[] = {"zero", "one", "two"};
static long counter;
long total(void) {
  long s = 0;
  for (int i = 0; i < 8; i++) s += table[i];
  return s;
}
int main(void) {
  for (int i = 0; i < 3; i++) printf("%s=%d\n", names[i], i);
  counter += total();
  printf("total %ld\n", counter);
  return 0;
}
