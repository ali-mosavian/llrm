// flags: -O2 -march=i486 | -O2 -march=i486 -m32
// data: @signal
/* Twin of stft_fixed.nib with its semantics: every index checked against the array length, and every fixed-point
   operation as Nib defines it (language-spec 3.4): `*` the full-width product shifted right by the fraction, `/` the dividend shifted left first. Short-time Fourier transform in Q9 fixed point, as stft_single.c: long is 32-bit, >> floors, as Nib's fixed '*' does. A product is
   (a * b) >> 9; samples are scaled by 1/4 and each stage by 1/2; a bin is ((re / 128)^2 + (im / 128)^2) / 256, at most
   65535. The twiddles are rotated in Q15 and cut to Q9. Prints the checksum of the values written:
   sum = (3 * sum + value + index) & 0x1FFFFFFF. */
extern void report(long value);
extern void ebnd(void);
#define CK(i, n) ((unsigned)(i) < (n) ? (i) : (ebnd(), 0))
#define BENCH_IO_FUNCTIONS /* the I/O calls are functions of this object, as Nib's std.io are of its */
#include "../../bench_io.h"

#define N 256
#define HOP 128
#define BINS 129
#define ONE 512L

static long qmul(long a, long b, int fraction) { return (long)(((long long)a * b) >> fraction); }
static long qdiv(long a, long b, int fraction) { return (long)(((long long)a << fraction) / b); }

static long tw_re[HOP], tw_im[HOP], window[N], re[N], im[N];
static unsigned char rev[N];
static short block[2 * HOP];
static unsigned short row[BINS];

/* The signal file is little-endian 16-bit samples; read into shorts it is byte-swapped per sample on a big-endian target. */
#if defined(__BYTE_ORDER__) && defined(__ORDER_BIG_ENDIAN__) && __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
static int bench_samples(int handle, short *to)
{
    int n = bench_read(handle, to, 2 * HOP), i;
    for (i = 0; i < HOP; ++i)
        to[i] = (short)((((unsigned short)to[i] & 255) << 8) | ((unsigned short)to[i] >> 8));
    return n;
}
#else
#define bench_samples(handle, to) bench_read(handle, to, 2 * HOP)
#endif

/* Nib's quarter(): a sample over 4 in Q9, cut to an integer and back. */
static long quarter(short sample) { return qdiv((long)sample * ONE, 4 * ONE, 9) / ONE * ONE; }

long bench_stft_fixed(void)
{
    long c = 32768L, s = 0, next, ar, ai, br, bi, tr, ti, pr, pi, sum = 0, index = 0, q;
    int in, out, n, k, i, j, m, half, stride, r, t, now = 0;

    for (k = 0; k < HOP; ++k) {
        tw_re[CK(k, HOP)] = c / 64;
        tw_im[CK(k, HOP)] = -(s / 64);
        next = qmul(c, 32758L, 15) - qmul(s, 804L, 15);
        s = qmul(s, 32758L, 15) + qmul(c, 804L, 15);
        c = next;
    }
    for (k = 0; k < HOP; ++k) {
        window[CK(k, N)] = qdiv(ONE - tw_re[CK(k, HOP)], 2 * ONE, 9);
        window[CK(k + HOP, N)] = ONE - window[CK(k, N)];
    }
    for (i = 0; i < N; ++i) {
        for (r = 0, t = i, j = 0; j < 8; ++j, t >>= 1)
            r = r * 2 + (t & 1);
        rev[CK(i, N)] = (unsigned char)r;
    }
    in = bench_open_read("SIGNAL");
    out = bench_create("SPECTRA");
    if (in < 0 || out < 0)
        return -1;
    if (bench_samples(in, block) != 2 * HOP)
        return -1;
    while (bench_samples(in, block + (1 - now) * HOP) == 2 * HOP) {
        for (n = 0; n < HOP; ++n) {
            re[CK(rev[CK(n, N)], N)] = qmul(quarter(block[CK(now * HOP + n, 2 * HOP)]), window[CK(n, N)], 9);
            re[CK(rev[CK(n + HOP, N)], N)] = qmul(quarter(block[CK((1 - now) * HOP + n, 2 * HOP)]), window[CK(n + HOP, N)], 9);
            im[CK(rev[CK(n, N)], N)] = 0;
            im[CK(rev[CK(n + HOP, N)], N)] = 0;
        }
        for (m = 2; m <= N; m *= 2) {
            half = m / 2;
            stride = N / m;
            for (i = 0; i < N; i += m) {
                for (j = 0; j < half; ++j) {
                    ar = re[CK(i + j, N)];
                    ai = im[CK(i + j, N)];
                    br = re[CK(i + j + half, N)];
                    bi = im[CK(i + j + half, N)];
                    tr = qmul(tw_re[CK(j * stride, HOP)], br, 9) - qmul(tw_im[CK(j * stride, HOP)], bi, 9);
                    ti = qmul(tw_re[CK(j * stride, HOP)], bi, 9) + qmul(tw_im[CK(j * stride, HOP)], br, 9);
                    re[CK(i + j, N)] = qdiv(ar + tr, 2 * ONE, 9);
                    im[CK(i + j, N)] = qdiv(ai + ti, 2 * ONE, 9);
                    re[CK(i + j + half, N)] = qdiv(ar - tr, 2 * ONE, 9);
                    im[CK(i + j + half, N)] = qdiv(ai - ti, 2 * ONE, 9);
                }
            }
        }
        for (k = 0; k < BINS; ++k) {
            pr = qmul(re[CK(k, N)], 4 * ONE, 9) / ONE; /* i32(re * 4) */
            pi = qmul(im[CK(k, N)], 4 * ONE, 9) / ONE;
            q = (pr * pr + pi * pi) / 256;
            if (q > 65535L)
                q = 65535L;
            row[CK(k, BINS)] = (unsigned short)q;
            sum = ((sum * 3) + q + index) & 0x1FFFFFFFL;
            ++index;
        }
        if (bench_write(out, row, 2 * BINS) != 2 * BINS)
            return -1;
        now = 1 - now;
    }
    bench_close(in);
    bench_close(out);
    return sum;
}

void ebnd(void) {}

int main(void)
{
    report(bench_stft_fixed());
    return 0;
}
