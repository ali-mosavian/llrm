// flags: -O2 -march=i486 | -O2 -march=i486 -m32
// data: @signal
/* Short-time Fourier transform in Q9 fixed point, as stft_single.c: long is 32-bit, >> floors, as Nib's fixed '*' does. A product is
   (a * b) >> 9; samples are scaled by 1/4 and each stage by 1/2; a bin is ((re / 128)^2 + (im / 128)^2) / 256, at most
   65535. The twiddles are rotated in Q15 and cut to Q9. Prints the checksum of the values written:
   sum = (3 * sum + value + index) & 0x1FFFFFFF. */
extern void report(long value);
#include "../bench_io.h"

#define N 256
#define HOP 128
#define BINS 129
#define ONE 512L

static long tw_re[HOP], tw_im[HOP], window[N], re[N], im[N];
static unsigned char rev[N];
static short block[2][HOP];
static unsigned short row[BINS];

long bench_stft_fixed(void)
{
    long c = 32768L, s = 0, next, ar, ai, br, bi, tr, ti, pr, pi, sum = 0, index = 0, q;
    int in, out, n, k, i, j, m, half, stride, r, t, now = 0;

    for (k = 0; k < HOP; ++k) {
        tw_re[k] = c / 64;
        tw_im[k] = -(s / 64);
        next = ((c * 32758L) >> 15) - ((s * 804L) >> 15);
        s = ((s * 32758L) >> 15) + ((c * 804L) >> 15);
        c = next;
    }
    for (k = 0; k < HOP; ++k) {
        window[k] = (ONE - tw_re[k]) / 2;
        window[k + HOP] = ONE - window[k];
    }
    for (i = 0; i < N; ++i) {
        for (r = 0, t = i, j = 0; j < 8; ++j, t >>= 1)
            r = r * 2 + (t & 1);
        rev[i] = (unsigned char)r;
    }
    in = bench_open_read("SIGNAL");
    out = bench_create("SPECTRA");
    if (in < 0 || out < 0)
        return -1;
    if (bench_read(in, block[now], 2 * HOP) != 2 * HOP)
        return -1;
    while (bench_read(in, block[1 - now], 2 * HOP) == 2 * HOP) {
        for (n = 0; n < HOP; ++n) {
            re[rev[n]] = (block[now][n] / 4L) * window[n];
            re[rev[n + HOP]] = (block[1 - now][n] / 4L) * window[n + HOP];
            im[rev[n]] = 0;
            im[rev[n + HOP]] = 0;
        }
        for (m = 2; m <= N; m *= 2) {
            half = m / 2;
            stride = N / m;
            for (i = 0; i < N; i += m) {
                for (j = 0; j < half; ++j) {
                    ar = re[i + j];
                    ai = im[i + j];
                    br = re[i + j + half];
                    bi = im[i + j + half];
                    tr = ((tw_re[j * stride] * br) >> 9) - ((tw_im[j * stride] * bi) >> 9);
                    ti = ((tw_re[j * stride] * bi) >> 9) + ((tw_im[j * stride] * br) >> 9);
                    re[i + j] = (ar + tr) / 2;
                    im[i + j] = (ai + ti) / 2;
                    re[i + j + half] = (ar - tr) / 2;
                    im[i + j + half] = (ai - ti) / 2;
                }
            }
        }
        for (k = 0; k < BINS; ++k) {
            pr = re[k] / 128;
            pi = im[k] / 128;
            q = (pr * pr + pi * pi) / 256;
            if (q > 65535L)
                q = 65535L;
            row[k] = (unsigned short)q;
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

int main(void)
{
    report(bench_stft_fixed());
    return 0;
}
