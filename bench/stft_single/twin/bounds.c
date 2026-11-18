// flags: -O2 -march=i486 | -O2 -march=i486 -m32
// data: @signal
/* Twin of stft_single.nib with its semantics: every index checked against the array length. Short-time Fourier transform in float: a 256-point Hann-windowed radix-2 FFT of every 128-sample hop of SIGNAL
   (16-bit little-endian samples, read one hop at a time); the 129 bin powers of each frame go to SPECTRA as 16-bit
   values. Samples are scaled by 1/4 and each stage by 1/2; a bin is (re^2 + im^2) / 16 truncated, at most 65535.
   Prints the checksum of the values written: sum = (3 * sum + value + index) & 0x1FFFFFFF. The twiddles are built
   by rotation, with no sin or cos. */
extern void report(long value);
extern void ebnd(void);
#define CK(i, n) ((unsigned)(i) < (n) ? (i) : (ebnd(), 0))
#define BENCH_IO_FUNCTIONS /* the I/O calls are functions of this object, as Nib's std.io are of its */
#include "../../bench_io.h"

#define N 256
#define HOP 128
#define BINS 129

static float tw_re[HOP], tw_im[HOP], window[N], re[N], im[N];
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

long bench_stft_single(void)
{
    float c = 1.0f, s = 0.0f, next, ar, ai, br, bi, tr, ti, p;
    long sum = 0, index = 0, q;
    int in, out, n, k, i, j, m, half, stride, r, t, now = 0;

    for (k = 0; k < HOP; ++k) {
        tw_re[CK(k, HOP)] = c;
        tw_im[CK(k, HOP)] = -s;
        next = c * 0.99969882f - s * 0.024541229f;
        s = s * 0.99969882f + c * 0.024541229f;
        c = next;
    }
    for (k = 0; k < HOP; ++k) {
        window[CK(k, N)] = (1.0f - tw_re[CK(k, HOP)]) * 0.5f;
        window[CK(k + HOP, N)] = 1.0f - window[CK(k, N)];
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
            re[CK(rev[CK(n, N)], N)] = (float)block[CK(now * HOP + n, 2 * HOP)] * 0.25f * window[CK(n, N)];
            re[CK(rev[CK(n + HOP, N)], N)] = (float)block[CK((1 - now) * HOP + n, 2 * HOP)] * 0.25f * window[CK(n + HOP, N)];
            im[CK(rev[CK(n, N)], N)] = 0.0f;
            im[CK(rev[CK(n + HOP, N)], N)] = 0.0f;
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
                    tr = tw_re[CK(j * stride, HOP)] * br - tw_im[CK(j * stride, HOP)] * bi;
                    ti = tw_re[CK(j * stride, HOP)] * bi + tw_im[CK(j * stride, HOP)] * br;
                    re[CK(i + j, N)] = (ar + tr) * 0.5f;
                    im[CK(i + j, N)] = (ai + ti) * 0.5f;
                    re[CK(i + j + half, N)] = (ar - tr) * 0.5f;
                    im[CK(i + j + half, N)] = (ai - ti) * 0.5f;
                }
            }
        }
        for (k = 0; k < BINS; ++k) {
            p = (re[CK(k, N)] * re[CK(k, N)] + im[CK(k, N)] * im[CK(k, N)]) * (1.0f / 16.0f);
            q = p >= 65535.0f ? 65535L : (long)p;
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
    report(bench_stft_single());
    return 0;
}
