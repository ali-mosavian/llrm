int printf(const char *, ...);
#define N 48
static double a[N][N], b[N][N], c[N][N];
int main(void) {
  for (int i = 0; i < N; i++) for (int j = 0; j < N; j++) { a[i][j] = i + j * 0.5; b[i][j] = i - j * 0.25; }
  for (int i = 0; i < N; i++) for (int j = 0; j < N; j++) { double s = 0; for (int k = 0; k < N; k++) s += a[i][k] * b[k][j]; c[i][j] = s; }
  double t = 0; for (int i = 0; i < N; i++) t += c[i][i];
  printf("%d\n", (int)t);
  return 0;
}
