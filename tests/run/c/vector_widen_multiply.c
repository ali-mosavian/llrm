// flags: -O2 | -O3
// bar: none (a correctness test: i16/i8 widening multiply loops at their extreme values, every trip count)
/* The vector loops multiply with `pmaddwd`, or `pmullw` with `pmulhw`/`pmulhuw` for the high words.
   A wrong signedness or a bad lane repack changes a product at -32768, 32767 or 65535. */
extern void report(long value);

#define N 27
static const int ws[] = {-32768, 32767, -1, 0, 1, -32767, 255, -256, 12345, -12345};
static const int bs[] = {-128, 127, -1, 0, 1, -127, 100, -100};
static short SA[N], SB[N];
static unsigned short UA[N], UB[N];
static signed char BA[N], BB[N];
static int SC[N];
static unsigned UC[N];

static long fold_s(int n) { unsigned s = 0; for (int i = 0; i < n; i++) s = s * 31 + (unsigned)SC[i]; return (long)(s & 0x7fffffff); }
static long fold_u(int n) { unsigned s = 0; for (int i = 0; i < n; i++) s = s * 31 + UC[i]; return (long)(s & 0x7fffffff); }

static long mul_s(int n) { for (int i = 0; i < n; i++) SC[i] = SA[i] * SB[i]; return fold_s(n); }
static long mul_u(int n) { for (int i = 0; i < n; i++) UC[i] = (unsigned)UA[i] * UB[i]; return fold_u(n); }
static long mul_b(int n) { for (int i = 0; i < n; i++) SC[i] = BA[i] * BB[i]; return fold_s(n); }
static long mul_su(int n) { for (int i = 0; i < n; i++) SC[i] = SA[i] * (int)UB[i]; return fold_s(n); }

int main(void)
{
    for (int i = 0; i < N; i++) {
        SA[i] = (short)ws[i % 10];
        SB[i] = (short)ws[(i * 7 + 3) % 10];
        UA[i] = (unsigned short)ws[(i * 3) % 10];
        UB[i] = (unsigned short)ws[(i * 5 + 1) % 10];
        BA[i] = (signed char)bs[i % 8];
        BB[i] = (signed char)bs[(i * 3 + 1) % 8];
    }
    for (int n = 0; n <= N; n++) {
        report(mul_s(n));
        report(mul_u(n));
        report(mul_b(n));
        report(mul_su(n));
    }
    return 0;
}
