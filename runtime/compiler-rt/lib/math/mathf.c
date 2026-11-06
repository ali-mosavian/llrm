/* binary32 libm subset: sqrtf, sinf, cosf, tanf, atanf, atan2f, asinf, acosf, expf, exp2f, logf, log2f, log10f.
 * Float arithmetic only (the target's soft float, or the host FPU in the test); coefficients in sf_coef.h.
 * sqrtf is exact (integer root). Measured max ulp over 24M inputs: sin/cos 3, tan 4, atan 2, asin/acos 3, exp/exp2/log 1,
 * log2/log10 2, atan2 2; the 3-4 are arguments beyond 2^20 (Payne-Hanek path), below 200 it is 2. */
#include "softfloat.h"
#include "sf_helpers.h"
#include "sf_coef.h"

#ifndef SFM_NAME
#define SFM_NAME(n) n
#endif

#define SIGN 0x80000000u
#define ABS(b) ((b) & ~SIGN)
#define INF_BITS 0x7F800000u

static inline float horner(const float *c, int n, float z)
{
    float p = c[n - 1];
    for (int i = n - 2; i >= 0; i--) p = p * z + c[i];
    return p;
}

static inline float pow2(int e) { return b2f((uint32_t)(e + 127) << 23); }
static inline float copysign_bits(float v, sf32 sign) { return b2f((f2b(v) & ~SIGN) | (sign & SIGN)); }
static inline int is_nan_bits(sf32 b) { return ABS(b) > INF_BITS; }

/* ---- sqrt: restoring integer root of the mantissa, correctly rounded ---- */

float SFM_NAME(sqrtf)(float x)
{
    sf32 b = f2b(x);
    if (is_nan_bits(b)) return b2f(b | 0x400000u);
    if (!ABS(b)) return x;
    if (b & SIGN) return b2f(SF32_QNAN);
    if (b == INF_BITS) return x;
    int e = (int)(b >> 23);
    uint32_t m = b & 0x7FFFFFu;
    if (e) m |= 0x800000u;
    else {
        int s = sf_clz32(m) - 8;
        m <<= s;
        e = 1 - s;
    }
    e -= 127;
    int odd = e & 1;
    /* q = floor(sqrt((m << odd) * 2^29)), 27 bits with the implicit one at bit 26. */
    uint32_t top = (m << odd) << 7, q = 0, r = 0;
    for (int i = 0; i < 27; i++) {
        r = (r << 2) | (top >> 30);
        top <<= 2;
        uint32_t t = (q << 2) | 1;
        q <<= 1;
        if (r >= t) { r -= t; q |= 1; }
    }
    return b2f(sf32_pack(0, (e - odd) / 2 + 127, q | (r != 0)));
}

/* ---- trig ---- */

static inline float sin_k(float x) { float z = x * x; return x + x * z * horner(SIN_P, SIN_P_N, z); }

static inline float cos_k(float x)
{
    float z = x * x, hz = 0.5f * z, w = 1.0f - hz;
    return w + (((1.0f - w) - hz) + z * z * horner(COS_P, COS_P_N, z));
}

static inline float tan_k(float x) { float z = x * x; return x + x * z * horner(TAN_P, TAN_P_N, z); }

/* Fraction of x * 2/pi from 96 bits of the table (Payne-Hanek); *np gets the nearest integer mod 4. */
static uint32_t table_word(int i) { return (i < 0 || i >= TWO_OVER_PI_WORDS) ? 0 : two_over_pi[i]; }

static uint32_t table_bits32(int p)
{
    int wi = p >> 5, sh = p & 31;
    uint32_t v = table_word(wi) << sh;
    return sh ? v | (table_word(wi + 1) >> (32 - sh)) : v;
}

static float reduce_large(float x, int *np)
{
    sf32 b = f2b(x);
    uint32_t m = (b & 0x7FFFFFu) | 0x800000u, h0, l0, h1, l1, h2, l2;
    int e2 = (int)((b >> 23) & 0xFF) - 150;
    /* G = bits e2-1 .. e2+94 of 2/pi; (m * G) mod 2^96, scaled 2^-94, is x*2/pi mod 4. */
    sf_mul32x32(m, table_bits32(e2 - 2), &h0, &l0);
    sf_mul32x32(m, table_bits32(e2 + 30), &h1, &l1);
    sf_mul32x32(m, table_bits32(e2 + 62), &h2, &l2);
    (void)h0; (void)l0;
    uint32_t q0 = l2, q1 = h2 + l1, c = q1 < h2, q2 = h1 + l0 + c;
    int n = (int)(q2 >> 30);
    q2 &= 0x3FFFFFFFu;
    int neg = 0;
    if (q2 & 0x20000000u) {
        /* fraction >= 1/2: round up, remainder is 2^94 - fraction, negative */
        n++;
        neg = 1;
        q0 = 0u - q0;
        q1 = ~q1 + (q0 == 0);
        q2 = (~q2 + (q0 == 0 && q1 == 0)) & 0x3FFFFFFFu;
    }
    /* The table gives x * 2/pi for x > 0; a negative x mirrors n and r. */
    int xneg = (b & SIGN) != 0;
    *np = (xneg ? -n : n) & 3;
    if (!(q0 | q1 | q2)) return 0.0f;
    int hb = q2 ? 64 + 31 - sf_clz32(q2) : q1 ? 32 + 31 - sf_clz32(q1) : 31 - sf_clz32(q0);
    int sh = 95 - hb;
    int ws = sh >> 5, bs = sh & 31;
    uint32_t w[3] = { q0, q1, q2 }, t = 0;
    /* top 32 bits of the 96-bit value shifted left by sh */
    int src = 2 - ws;
    if (src >= 0) t = w[src] << bs;
    if (bs && src - 1 >= 0) t |= w[src - 1] >> (32 - bs);
    float f = (float)t;
    f = b2f(f2b(f) - ((uint32_t)(30 + sh) << 23));
    f = f * PIO2_HI + f * PIO2_LO;
    return neg != xneg ? -f : f;
}

/* x = r + n * pi/2, |r| <= ~pi/4. */
static float reduce(float x, int *np)
{
    sf32 a = ABS(f2b(x));
    if (a <= 0x3F490FDBu) { *np = 0; return x; }
    if (a < 0x43490000u) {
        float fx = x * TWO_OVER_PI;
        int k = (int)(fx + (fx < 0 ? -0.5f : 0.5f));
        float kf = (float)k;
        float r = ((x - kf * PIO2_1) - kf * PIO2_2) - kf * PIO2_3;
        if (ABS(f2b(r)) >= 0x39800000u) { *np = k & 3; return r; }   /* |r| >= 2^-12 */
    }
    return reduce_large(x, np);
}

float SFM_NAME(sinf)(float x)
{
    sf32 b = f2b(x), a = ABS(b);
    if (a >= INF_BITS) return a == INF_BITS ? b2f(SF32_QNAN) : b2f(b | 0x400000u);
    if (a < 0x39000000u) return x;
    int n;
    float r = reduce(x, &n);
    switch (n) {
    case 0: return sin_k(r);
    case 1: return cos_k(r);
    case 2: return -sin_k(r);
    default: return -cos_k(r);
    }
}

float SFM_NAME(cosf)(float x)
{
    sf32 b = f2b(x), a = ABS(b);
    if (a >= INF_BITS) return a == INF_BITS ? b2f(SF32_QNAN) : b2f(b | 0x400000u);
    if (a < 0x32000000u) return 1.0f;
    int n;
    float r = reduce(x, &n);
    switch (n) {
    case 0: return cos_k(r);
    case 1: return -sin_k(r);
    case 2: return -cos_k(r);
    default: return sin_k(r);
    }
}

float SFM_NAME(tanf)(float x)
{
    sf32 b = f2b(x), a = ABS(b);
    if (a >= INF_BITS) return a == INF_BITS ? b2f(SF32_QNAN) : b2f(b | 0x400000u);
    if (a < 0x39000000u) return x;
    int n;
    float r = reduce(x, &n);
    return (n & 1) ? -1.0f / tan_k(r) : tan_k(r);
}

/* ---- inverse trig ---- */

static inline float atan_k(float x) { float z = x * x; return x + x * z * horner(ATAN_P, ATAN_P_N, z); }

/* atan of a non-negative finite or infinite value. */
static float atan_pos(float ax)
{
    if (ax <= TAN_PI_8) return atan_k(ax);
    if (ax <= TAN_3PI_8) {
        float t = (ax - 1.0f) / (ax + 1.0f);
        return PIO4_HI + (atan_k(t) + PIO2_LO * 0.5f);
    }
    return PIO2_HI - (atan_k(1.0f / ax) - PIO2_LO);
}

float SFM_NAME(atanf)(float x)
{
    sf32 b = f2b(x), a = ABS(b);
    if (is_nan_bits(b)) return b2f(b | 0x400000u);
    if (a < 0x39000000u) return x;
    return copysign_bits(atan_pos(b2f(a)), b);
}

float SFM_NAME(atan2f)(float y, float x)
{
    sf32 by = f2b(y), bx = f2b(x), ay = ABS(by), ax = ABS(bx);
    if (is_nan_bits(by) || is_nan_bits(bx)) return b2f((is_nan_bits(by) ? by : bx) | 0x400000u);
    if (!ay) return (bx & SIGN) ? copysign_bits(PI_HI, by) : y;
    if (!ax) return copysign_bits(PIO2_HI, by);
    if (ax == INF_BITS) {
        if (ay == INF_BITS) return copysign_bits((bx & SIGN) ? 3.0f * PIO4_HI : PIO4_HI, by);
        return (bx & SIGN) ? copysign_bits(PI_HI, by) : copysign_bits(0.0f, by);
    }
    if (ay == INF_BITS) return copysign_bits(PIO2_HI, by);
    float w = atan_pos(b2f(ay) / b2f(ax));
    if (bx & SIGN) w = PI_HI - (w - PI_LO);
    return copysign_bits(w, by);
}

float SFM_NAME(asinf)(float x)
{
    sf32 b = f2b(x), a = ABS(b);
    if (is_nan_bits(b)) return b2f(b | 0x400000u);
    if (a > 0x3F800000u) return b2f(SF32_QNAN);
    if (a < 0x39000000u) return x;
    return SFM_NAME(atan2f)(x, SFM_NAME(sqrtf)((1.0f - x) * (1.0f + x)));
}

float SFM_NAME(acosf)(float x)
{
    sf32 b = f2b(x), a = ABS(b);
    if (is_nan_bits(b)) return b2f(b | 0x400000u);
    if (a > 0x3F800000u) return b2f(SF32_QNAN);
    return SFM_NAME(atan2f)(SFM_NAME(sqrtf)((1.0f - x) * (1.0f + x)), x);
}

/* ---- exp and log ---- */

/* p * 2^k for |k| up to 150 with a single final rounding. */
static float scale2(float p, int k)
{
    if (k > 127) return (p * pow2(127)) * 2.0f;
    if (k < -125) return (p * pow2(k + 100)) * pow2(-100);
    return p * pow2(k);
}

static float exp_core(float r, int k)
{
    float p = 1.0f + (r + r * r * horner(EXP_P, EXP_P_N, r));
    return scale2(p, k);
}

float SFM_NAME(expf)(float x)
{
    sf32 b = f2b(x), a = ABS(b);
    if (is_nan_bits(b)) return b2f(b | 0x400000u);
    if (x > 88.7228394f) return b2f(INF_BITS);
    if (x < -103.972084f) return 0.0f;
    if (a < 0x32000000u) return 1.0f + x;
    int k = (int)(x * LOG2E + (x < 0 ? -0.5f : 0.5f));
    float kf = (float)k;
    return exp_core((x - kf * LN2_1) - kf * LN2_2, k);
}

float SFM_NAME(exp2f)(float x)
{
    sf32 b = f2b(x);
    if (is_nan_bits(b)) return b2f(b | 0x400000u);
    if (x >= 128.0f) return b2f(INF_BITS);
    if (x < -150.0f) return 0.0f;
    int k = (int)(x + (x < 0 ? -0.5f : 0.5f));
    float u = x - (float)k;
    return exp_core(u * LN2_1 + u * LN2_2, k);
}

/* Pieces of log(1+f) = f - hfsq + s * (hfsq + R) for f in [sqrt(1/2)-1, sqrt(2)-1]. */
static void log1p_part(float f, float *hfsq_out, float *s_out, float *R_out)
{
    float s = f / (2.0f + f), z = s * s;
    *R_out = z * horner(LOG_P, LOG_P_N, z);
    *hfsq_out = 0.5f * f * f;
    *s_out = s;
}

/* x = 2^k * (1 + f), f in [sqrt(1/2)-1, sqrt(2)-1]; x positive finite. */
static float log_split(float x, int *kp)
{
    sf32 b = f2b(x);
    int k = 0;
    if (b < 0x00800000u) { b = f2b(x * 33554432.0f); k = -25; }
    k += (int)(b >> 23) - 127;
    b = (b & 0x7FFFFFu) | 0x3F800000u;
    if (b > 0x3FB504F3u) { b -= 0x00800000u; k++; }
    *kp = k;
    return b2f(b) - 1.0f;
}

float SFM_NAME(logf)(float x)
{
    sf32 b = f2b(x);
    if (is_nan_bits(b)) return b2f(b | 0x400000u);
    if (!ABS(b)) return -b2f(INF_BITS);
    if (b & SIGN) return b2f(SF32_QNAN);
    if (b == INF_BITS) return x;
    int k;
    float f = log_split(x, &k), hfsq, s, R;
    log1p_part(f, &hfsq, &s, &R);
    if (!k) return f - (hfsq - s * (hfsq + R));
    float dk = (float)k;
    return dk * LN2_1 - ((hfsq - (s * (hfsq + R) + dk * LN2_2)) - f);
}

float SFM_NAME(log2f)(float x)
{
    sf32 b = f2b(x);
    if (is_nan_bits(b)) return b2f(b | 0x400000u);
    if (!ABS(b)) return -b2f(INF_BITS);
    if (b & SIGN) return b2f(SF32_QNAN);
    if (b == INF_BITS) return x;
    int k;
    float f = log_split(x, &k), hfsq, s, R;
    log1p_part(f, &hfsq, &s, &R);
    return (float)k + (f - (hfsq - s * (hfsq + R))) * LOG2E;
}

float SFM_NAME(log10f)(float x) { return SFM_NAME(logf)(x) * LOG10E; }
