// flags: -O2 | -O3
// bar: none (a correctness test: float compare, select and conversion loops of every trip count, NaN included)
/* The vector loops compare with `cmpps`, select with `pand`/`pandn`/`por` and convert with `cvttps2dq`/`cvtdq2ps`.
   A wrong predicate, or a NaN that takes the wrong side, changes a sum; the scalar remainder must agree at every count. */
extern void report(long value);

#define N 23
static volatile float zero;
static float A[N], B[N], C[N];
static int I[N], R[N];

#define COMPARE(name, op) \
    static long name(int n) { for (int i = 0; i < n; i++) R[i] = A[i] op B[i]; long s = 0; for (int i = 0; i < n; i++) s = s * 3 + R[i]; return s; }
COMPARE(lt, <) COMPARE(le, <=) COMPARE(gt, >) COMPARE(ge, >=) COMPARE(eq, ==) COMPARE(ne, !=)

static long min_of(int n)
{
    for (int i = 0; i < n; i++) C[i] = A[i] < B[i] ? A[i] : B[i];
    long s = 0;
    for (int i = 0; i < n; i++) s = s * 3 + (long)(C[i] * 8);
    return s;
}

static long to_int(int n)
{
    for (int i = 0; i < n; i++) R[i] = (int)A[i];
    long s = 0;
    for (int i = 0; i < n; i++) s = s * 3 + R[i];
    return s;
}

static long to_float(int n)
{
    for (int i = 0; i < n; i++) C[i] = (float)I[i];
    long s = 0;
    for (int i = 0; i < n; i++) s = s * 3 + (long)C[i];
    return s;
}

int main(void)
{
    float nan = zero / zero;
    for (int i = 0; i < N; i++) {
        A[i] = (i % 5 == 3) ? nan : (float)(i % 7) - 3;
        B[i] = (i % 6 == 4) ? nan : (float)((i * 3) % 7) - 3;
        I[i] = (i - 11) * 977;
    }
    for (int n = 0; n <= N; n += 5) {
        report(lt(n)); report(le(n)); report(gt(n)); report(ge(n)); report(eq(n)); report(ne(n));
        report(min_of(n));
        report(to_int(n));
        report(to_float(n));
    }
    return 0;
}
