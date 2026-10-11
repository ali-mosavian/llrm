// flags: -O2 -march=i486 | -O2 -march=i486 -m32
// data: @signal
/* Short-time Fourier transform in float: a 256-point Hann-windowed radix-2 FFT of every 128-sample hop of SIGNAL
   (16-bit little-endian samples, read one hop at a time); the 129 bin powers of each frame go to SPECTRA as 16-bit
   values. Samples are scaled by 1/4 and each stage by 1/2; a bin is (re^2 + im^2) / 16 truncated, at most 65535.
   Prints the checksum of the values written: sum = (3 * sum + value + index) & 0x1FFFFFFF. The twiddles are built
   by rotation, with no sin or cos. */
extern void report(long value);
#include "../bench_io.h"

#define N 256
#define HOP 128
#define BINS 129

static float tw_re[HOP], tw_im[HOP], window[N], re[N], im[N];
static unsigned char rev[N];
static short block[2][HOP];
static unsigned short row[BINS];

long bench_stft_single(void)
{
    float c = 1.0f, s = 0.0f, next, ar, ai, br, bi, tr, ti, p;
    long sum = 0, index = 0, q;
    int in, out, n, k, i, j, m, half, stride, r, t, now = 0;

    for (k = 0; k < HOP; ++k) {
        tw_re[k] = c;
        tw_im[k] = -s;
        next = c * 0.99969882f - s * 0.024541229f;
        s = s * 0.99969882f + c * 0.024541229f;
        c = next;
    }
    for (k = 0; k < HOP; ++k) {
        window[k] = (1.0f - tw_re[k]) * 0.5f;
        window[k + HOP] = 1.0f - window[k];
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
            re[rev[n]] = (float)block[now][n] * 0.25f * window[n];
            re[rev[n + HOP]] = (float)block[1 - now][n] * 0.25f * window[n + HOP];
            im[rev[n]] = 0.0f;
            im[rev[n + HOP]] = 0.0f;
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
                    tr = tw_re[j * stride] * br - tw_im[j * stride] * bi;
                    ti = tw_re[j * stride] * bi + tw_im[j * stride] * br;
                    re[i + j] = (ar + tr) * 0.5f;
                    im[i + j] = (ai + ti) * 0.5f;
                    re[i + j + half] = (ar - tr) * 0.5f;
                    im[i + j + half] = (ai - ti) * 0.5f;
                }
            }
        }
        for (k = 0; k < BINS; ++k) {
            p = (re[k] * re[k] + im[k] * im[k]) * (1.0f / 16.0f);
            q = p >= 65535.0f ? 65535L : (long)p;
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
    report(bench_stft_single());
    return 0;
}
