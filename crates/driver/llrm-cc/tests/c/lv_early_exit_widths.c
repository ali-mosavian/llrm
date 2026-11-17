unsigned short a[1200], b[1200];
int first_below(void) {
  for (int i = 0; i < 1200; i++)
    if (a[i] < b[i]) return 1;
  return 0;
}
int c[1200], d[1200];
int first_differs(void) {
  for (int i = 0; i < 1200; i++)
    if (c[i] != d[i]) return 1;
  return 0;
}
signed char e[1200], f[1200];
int first_greater(void) {
  for (int i = 0; i < 1200; i++)
    if (e[i] > f[i]) return 1;
  return 0;
}
