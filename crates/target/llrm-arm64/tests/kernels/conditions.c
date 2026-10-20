int printf(const char *, ...);
static int v[4096];
int main(void) {
  unsigned s = 12345;
  for (int i = 0; i < 4096; i++) { s = s * 1103515245u + 12345u; v[i] = (int)(s >> 16) & 1023; }
  int a = 0, b = 0, c = 0;
  for (int i = 0; i < 4096; i++) {
    int x = v[i];
    if (x > 100 && x < 900) a++;
    if (x == 7 || x == 99 || x == 513) b++;
    if ((x & 1) && x > 500) c += x;
    if (x >= 10 && x <= 20 && (x & 3)) c -= 1;
  }
  printf("%d %d %d\n", a, b, c);
  return 0;
}
