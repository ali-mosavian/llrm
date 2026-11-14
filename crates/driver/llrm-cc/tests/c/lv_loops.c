int a[1024], b[1024];
void add_arrays(void) { for (int i = 0; i < 1024; ++i) a[i] += b[i]; }
void copy_plus_one(int *d, const int *s, int n) { for (int i = 0; i < n; ++i) d[i] = s[i] + 1; }
void multiply_arrays(void) { for (int i = 0; i < 1024; ++i) a[i] *= b[i]; }

int sum_array(const int *a, int n) {
  int total = 0;
  for (int i = 0; i < n; i++) total += a[i];
  return total;
}
