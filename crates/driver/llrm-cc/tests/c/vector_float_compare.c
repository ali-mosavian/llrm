/* x86-64 -O2: loops of float compares, selects and conversions are vector loops (`cmpps`, `cvttps2dq`, `cvtdq2ps`, `pandn`).
 * Before, the compare and the strided float loads had no vector price, so the loop stayed scalar. */
struct rec { int pad0; float a; float b; int pad1[8]; };

void compare_lt(int *restrict out, const float *restrict a, const float *restrict b, int n)
{
    for (int i = 0; i < n; i++) out[i] = a[i] < b[i];
}

void select_min(float *restrict out, const float *restrict a, const float *restrict b, int n)
{
    for (int i = 0; i < n; i++) out[i] = a[i] < b[i] ? a[i] : b[i];
}

void float_to_int(int *restrict out, const float *restrict a, int n)
{
    for (int i = 0; i < n; i++) out[i] = (int)a[i];
}

void int_to_float(float *restrict out, const int *restrict a, int n)
{
    for (int i = 0; i < n; i++) out[i] = (float)a[i];
}

/* Lanes 44 bytes apart: four loads and their unpacks, not a spill and a reload. */
int strided_sum(const struct rec *r, int n)
{
    int s = 0;
    for (int i = 0; i < n; i++) s += (int)r[i].a;
    return s;
}
