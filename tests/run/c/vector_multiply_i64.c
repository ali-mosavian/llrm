// flags: -O2 | -O3
// bar: none (a correctness test: 64-bit lane multiplies and dot products at their extreme values, every trip count)
/* A `<2 x i64>` multiply is three `pmuludq`, shifts and adds (a half known to be zero leaves a product out); an `i16` dot product
   into 64 bits is `pmullw`/`pmulhw` and a sign extension. A wrong half or sign changes a hash. */
extern void report(long value);

#define N 21
static const long long vs[] = {0LL, 1LL, -1LL, 2147483647LL, -2147483648LL, 4294967295LL, 4294967296LL, 9223372036854775807LL,
                               -9223372036854775807LL - 1, 123456789012345LL, -98765432109876LL, 65535LL};
static const short ws[] = {-32768, 32767, -1, 0, 1, -32767, 255, -256, 12345, -12345};
static long long A[N], B[N], C[N];
static unsigned UA[N], UB[N];
static int SA[N], SB[N];
static short HA[N], HB[N];

static long hash(const long long *p, int n)
{
    unsigned long long h = 0;
    for (int i = 0; i < n; i++) h = h * 1000003ULL + (unsigned long long)p[i];
    return (long)((h >> 7) & 0x7fffffff);
}

static long mul64(int n) { for (int i = 0; i < n; i++) C[i] = A[i] * B[i]; return hash(C, n); }
static long mul_unsigned(int n) { for (int i = 0; i < n; i++) C[i] = (long long)((unsigned long long)UA[i] * UB[i]); return hash(C, n); }
static long mul_signed(int n) { for (int i = 0; i < n; i++) C[i] = (long long)SA[i] * SB[i]; return hash(C, n); }
static long mul_mixed(int n) { for (int i = 0; i < n; i++) C[i] = (long long)SA[i] * UB[i]; return hash(C, n); }
static long dot(int n) { long long s = 0; for (int i = 0; i < n; i++) s += (long long)HA[i] * HB[i]; return (long)(s & 0x7fffffff); }

int main(void)
{
    for (int i = 0; i < N; i++) {
        A[i] = vs[i % 12];
        B[i] = vs[(i * 5 + 3) % 12];
        UA[i] = (unsigned)vs[(i * 7 + 1) % 12];
        UB[i] = (unsigned)vs[(i * 3 + 2) % 12];
        SA[i] = (int)vs[(i * 7 + 1) % 12];
        SB[i] = (int)vs[(i * 5 + 4) % 12];
        HA[i] = ws[i % 10];
        HB[i] = ws[(i * 7 + 3) % 10];
    }
    for (int n = 0; n <= N; n++) {
        report(mul64(n));
        report(mul_unsigned(n));
        report(mul_signed(n));
        report(mul_mixed(n));
        report(dot(n));
    }
    return 0;
}
