/* Arithmetic-only reference. See README.md before using its instruction count. */
#pragma STDC FENV_ACCESS ON

void fpcsex(float a, float b, float c,
            float *restrict p, float *restrict q, float *restrict sum)
{
    for (int iteration = 0; iteration < 10; ++iteration) {
        *p = ((long double)a + b) * c;
        *q = ((long double)a + b) / c;
        *sum = ((long double)*sum + *p) + *q;
    }
}
