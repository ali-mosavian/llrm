int a[1024], b[1024];
void add_arrays(void) { for (int i = 0; i < 1024; ++i) a[i] += b[i]; }
void copy_plus_one(int *d, const int *s, int n) { for (int i = 0; i < n; ++i) d[i] = s[i] + 1; }
void multiply_arrays(void) { for (int i = 0; i < 1024; ++i) a[i] *= b[i]; }

int sum_array(const int *a, int n) {
  int total = 0;
  for (int i = 0; i < n; i++) total += a[i];
  return total;
}

void fill_rows(short t[64][64]) {
  for (int y = 0; y < 64; ++y)
    for (int x = 0; x < 64; ++x) t[y][x] = 4660;
}

/* Vectorized, they left `bitcast (<16 x i8> ... to i128)` and `bitcast (i128 N to <4 x i32>)` constants that isel refused:
   CannotSelect("Truncate"), "a vector constant that is not a list of integers". */
int stack_copy(int n) {
  char a[70], b[70];
  for (int i = 0; i < 70; i++) a[i] = (char)i;
  for (int i = 0; i < (n & 63); i++) b[i] = a[i];
  return b[3];
}
int square_sum(int n) {
  int a[n], s = 0;
  for (int i = 0; i < n; i++) a[i] = i * i;
  for (int i = 0; i < n; i++) s += a[i];
  return s;
}
