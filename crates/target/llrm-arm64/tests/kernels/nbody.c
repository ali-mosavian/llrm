int printf(const char *, ...);
double sqrt(double);
#define B 5
static double x[B], y[B], z[B], vx[B], vy[B], vz[B], m[B];
int main(void) {
  for (int i = 0; i < B; i++) { x[i] = i * 1.5; y[i] = i * 0.5 - 1; z[i] = i * 0.25; m[i] = 1 + i; }
  for (int step = 0; step < 2000; step++) {
    for (int i = 0; i < B; i++) for (int j = i + 1; j < B; j++) {
      double dx = x[i] - x[j], dy = y[i] - y[j], dz = z[i] - z[j];
      double d2 = dx * dx + dy * dy + dz * dz + 0.01, mag = 0.01 / (d2 * __builtin_sqrt(d2));
      vx[i] -= dx * m[j] * mag; vy[i] -= dy * m[j] * mag; vz[i] -= dz * m[j] * mag;
      vx[j] += dx * m[i] * mag; vy[j] += dy * m[i] * mag; vz[j] += dz * m[i] * mag;
    }
    for (int i = 0; i < B; i++) { x[i] += 0.01 * vx[i]; y[i] += 0.01 * vy[i]; z[i] += 0.01 * vz[i]; }
  }
  printf("%d\n", (int)(x[0] * 1000));
  return 0;
}
