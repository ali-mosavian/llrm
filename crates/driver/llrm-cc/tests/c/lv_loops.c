int a[1024], b[1024];
void add_arrays(void) { for (int i = 0; i < 1024; ++i) a[i] += b[i]; }
void copy_plus_one(int *d, const int *s, int n) { for (int i = 0; i < n; ++i) d[i] = s[i] + 1; }
void multiply_arrays(void) { for (int i = 0; i < 1024; ++i) a[i] *= b[i]; }
void times_three(void) { for (int i = 0; i < 1024; ++i) a[i] = b[i] * 3; }
void times_seven(void) { for (int i = 0; i < 1024; ++i) a[i] = b[i] * 7; }
void times_minus_three(void) { for (int i = 0; i < 1024; ++i) a[i] = b[i] * -3; }

int sum_array(const int *a, int n) {
  int total = 0;
  for (int i = 0; i < n; i++) total += a[i];
  return total;
}

long sum_longs(const long *a, int n) { long t = 0; for (int i = 0; i < n; i++) t += a[i]; return t; }
short sum_shorts(const short *a, int n) { short t = 0; for (int i = 0; i < n; i++) t += a[i]; return t; }
signed char sum_bytes(const signed char *a, int n) { signed char t = 0; for (int i = 0; i < n; i++) t += a[i]; return t; }

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

short sa[1024], sb[1024];
void multiply_shorts(void) { for (int i = 0; i < 1024; ++i) sa[i] *= sb[i]; }

int si[1024];
unsigned ui[1024];
void shift_ints(void) { for (int i = 0; i < 1024; ++i) si[i] = (si[i] << 3) + (si[i] >> 2) + (int)(ui[i] >> 5); }

void fill_value(int *a, int v, int n) { for (int i = 0; i < n; ++i) a[i] = v; }
void fill_long_value(long *a, long v, int n) { for (int i = 0; i < n; ++i) a[i] = v; }

unsigned char pa[4096], pb[4096];
void multiply_bytes(void) { for (unsigned long i = 0; i < 4096; ++i) pa[i] = pb[i] = (unsigned char)(i * 13 + 5); }
void multiply_byte_arrays(unsigned char *x, const unsigned char *y, const unsigned char *z) {
  for (int i = 0; i < 4096; ++i) x[i] = y[i] * z[i];
}

void add_five(void) { for (int i = 0; i < 1024; ++i) a[i] += 5; }

/* A byte accumulator seeded from an argument and folded with a loop-invariant byte: the seed is lane 0 of a zero
   vector and the operand a broadcast (`insertelement`), which had no x86 lowering (known_loads_walks). */
unsigned char xor_toggle_count(unsigned char n)
{
    unsigned char x = n;
    for (unsigned char i = 0; i < n; i++)
        x ^= 1;
    return x;
}

/* A byte table computed in 32-bit arithmetic: the narrowing to bytes (`trunc <16 x i32> to <16 x i8>`) was unpriced,
   so the loop stayed scalar (mem_length_boundaries_m32: +70% executed instructions). */
void pattern_bytes(unsigned char *dst, unsigned n)
{
    for (unsigned i = 0; i < n; i++)
        dst[i] = (unsigned char)((i & 3) + ((i >> 2) & 1));
}

/* A count-down sum with a run-time start: the vector loop's scalar remainder is marked
   `llvm.loop.unroll.runtime.disable`, which the unroller read as absent (loop_countdown_sum). */
unsigned down_sum(int n)
{
    unsigned s = 0;
    for (int i = n; i >= 0; i--)
        s += (((unsigned)i * 3u) ^ ((unsigned)i >> 2));
    return s;
}

/* A 32-trip fold of a table: fourteen units a trip, 386 unrolled, over the pin's -O3 threshold of 300
   (loop_interchange's `checksum`). */
long table_fold(const short *table)
{
    long sum = 0;
    for (int i = 0; i < 32; ++i)
        sum = (sum * 31 + table[i]) % 100003;
    return sum;
}

/* The weight `(i & 15) + 1` of an `i64` counter fits 16 bits: gcc and the narrowed chain keep it in `<8 x i16>`. */
long weighted_sum(const short *a)
{
    long total = 0;
    for (long i = 0; i < 2000; ++i) total += (long)a[i] * ((i & 15) + 1);
    return total;
}

/* tile's init: `(x * 3 + y * 5) & 255` of an `i64` counter stored as `short`: the trunc of the splat add is `<8 x i16>`. */
void init_tile(short t[64][64])
{
    for (long y = 0; y < 64; ++y)
        for (long x = 0; x < 64; ++x) t[y][x] = (x * 3 + y * 5) & 255;
}
