int printf(const char *, ...);
static double mandel(double cr, double ci) {
  double zr = 0, zi = 0;
  int i;
  for (i = 0; i < 50 && zr * zr + zi * zi < 4; i++) { double t = zr * zr - zi * zi + cr; zi = 2 * zr * zi + ci; zr = t; }
  return i;
}
static double sqrt_newton(double x) { double g = x / 2; for (int i = 0; i < 20; i++) g = (g + x / g) / 2; return g; }
static void matmul(const double a[3][3], const double b[3][3], double c[3][3]) {
  for (int i = 0; i < 3; i++) for (int j = 0; j < 3; j++) { double s = 0; for (int k = 0; k < 3; k++) s += a[i][k] * b[k][j]; c[i][j] = s; }
}
static unsigned crc32(const unsigned char *p, int n) {
  unsigned c = 0xffffffffu;
  for (int i = 0; i < n; i++) { c ^= p[i]; for (int k = 0; k < 8; k++) c = c & 1 ? (c >> 1) ^ 0xedb88320u : c >> 1; }
  return ~c;
}
int main(void) {
  for (int y = -6; y <= 6; y += 3) { for (int x = -10; x <= 4; x += 2) printf("%c", " .:-=+*#%@"[(int)mandel(x / 5.0, y / 5.0) % 10]); printf("\n"); }
  printf("%d %d\n", (int)(sqrt_newton(2) * 1000000), (int)(sqrt_newton(10) * 1000));
  double a[3][3] = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}}, b[3][3] = {{9, 8, 7}, {6, 5, 4}, {3, 2, 1}}, c[3][3];
  matmul(a, b, c);
  for (int i = 0; i < 3; i++) printf("%d %d %d\n", (int)c[i][0], (int)c[i][1], (int)c[i][2]);
  unsigned char data[] = "The quick brown fox jumps over the lazy dog";
  printf("%x\n", crc32(data, 43));
  return 0;
}
