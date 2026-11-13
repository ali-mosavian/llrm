// flags: -O2 -march=i486 | -O2 -march=i486 -m32
// data: ../../jpeg_single/photo.jpg
/* Twin of jpeg_fixed.nib with its semantics: every index checked against the array length of the Nib program (its flat arrays, the BMP header built in line), local arrays zeroed. JPEG to BMP, an MCU row at a time: baseline Huffman YCbCr in 4:2:0, 4:4:4, 4:2:2 or 4:4:0, with restart markers. Reads
   PHOTO.JPG, writes a top-down 24-bit OUT.BMP and returns the Adler-32 of the BMP's bytes. The IDCT and the colour
   conversion are in 32-bit Q10 fixed point (long), each product truncated toward zero; chroma is replicated. A progressive or arithmetic-coded file is refused with a message. */
#define BENCH_IO_FUNCTIONS /* the I/O calls are functions of this object, as Nib's std.io are of its */
#include "../../bench_io.h"
extern void report(long value);
extern void ebnd(void);
#define CK(i, n) ((unsigned)(i) < (n) ? (i) : (ebnd(), 0))

#define MAXW 256 /* the widest picture: a row of MCUs is buffered */

/* Q10: ten fraction bits. cos((2x+1)u*pi/16) * (u ? 1 : 1/sqrt(2)) / 2 */
static const int cosine[64] = {
    362, 502, 473, 426, 362, 284, 196, 100,       362, 426, 196, -100, -362, -502, -473, -284, 362, 284, -196, -502, -362, 100, 473, 426,
    362, 100, -473, -284, 362, 426, -196, -502,   362, -100, -473, 284, 362, -426, -196, 502,  362, -284, -196, 502, -362, -100, 473, -426,
    362, -426, 196, 100, -362, 502, -473, 284,    362, -502, 473, -426, 362, -284, 196, -100};
static const unsigned char zigzag[64] = {0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28,
                                         35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63};

static int infile, bad;
static unsigned char inbuf[512], planes[12288], line[MAXW * 3 + 3];
static int inpos, inlen, bitbuf, bitcnt;
static int quant[256], maxcode[72], valptr[72], mincode[72];
static unsigned char huffval[1024];
static long coef[64];
static unsigned long adler_a = 1, adler_b;

static int next(void)
{
    int inbuf_byte;
    if (inpos == inlen) {
        inlen = bench_read(infile, inbuf, sizeof inbuf);
        inpos = 0;
        if (inlen <= 0) {
            inlen = 0;
            bad = 1;
            return 0;
        }
    }
    inbuf_byte = inbuf[CK(inpos, 512)];
    ++inpos;
    return inbuf_byte;
}

static int next16(void)
{
    int high = next();
    return high * 256 + next();
}

static int getbit(void)
{
    if (bitcnt == 0) {
        bitbuf = next();
        if (bitbuf == 0xFF && next() != 0)
            bad = 1; /* a marker inside the data */
        bitcnt = 8;
    }
    --bitcnt;
    return (bitbuf >> bitcnt) & 1;
}

static int receive(int count)
{
    int value = 0;
    while (count--)
        value = value * 2 + getbit();
    return value;
}

static int decode(int table)
{
    int code = getbit(), length = 1;
    while (code > maxcode[CK(table * 18 + length, 72)]) {
        code = code * 2 + getbit();
        if (++length > 16) {
            bad = 1;
            return 0;
        }
    }
    return huffval[CK(table * 256 + valptr[CK(table * 18 + length, 72)] + code - mincode[CK(table * 18 + length, 72)], 1024)];
}

static int extend(int value, int size)
{
    return value >= (1 << (size - 1)) ? value : value - (1 << size) + 1;
}

static void restart(void)
{
    int marker;
    bitcnt = 0;
    do {
        while (next() != 0xFF && !bad)
            ;
        marker = next();
    } while (!bad && (marker < 0xD0 || marker > 0xD7));
}

static void emit(int out, const unsigned char *data, int count)
{
    int i;
    bench_write(out, data, count);
    for (i = 0; i < count; ++i) {
        adler_a += data[CK(i, 771)];
        if (adler_a >= 65521UL)
            adler_a -= 65521UL;
        adler_b += adler_a;
        if (adler_b >= 65521UL)
            adler_b -= 65521UL;
    }
}

static void put32(int at, unsigned long value)
{
    int i;
    for (i = 0; i < 4; ++i)
        line[CK(at + i, 771)] = (unsigned char)(value >> (8 * i));
}

static void message(const char *text)
{
    int length = 0;
    while (text[length])
        ++length;
    bench_write(1, text, length);
}

/* Nib's tmul: the Q10 product, full width (a 64-bit intermediate, as the language defines `*`), truncated toward zero by the sign cases. */
static long qmul(long a, long b)
{
    return (long)(((long long)a * b) >> 10);
}

static long tmul(long a, long b)
{
    if (a < 0)
        return 0 - tmul(0 - a, b);
    if (b < 0)
        return 0 - qmul(a, 0 - b);
    return qmul(a, b);
}

/* The 8x8 block in coef, into plane `p` at (x0, y0): rows to Q10, then columns, +128.5, to integers. */
static void idct(int p, int x0, int y0)
{
    long tmp[64] = {0}, sum;
    int v, x, u, y, value;

    for (v = 0; v < 8; ++v)
        for (x = 0; x < 8; ++x) {
            sum = 0;
            for (u = 0; u < 8; ++u)
                sum += tmul(cosine[CK(x * 8 + u, 64)], coef[CK(v * 8 + u, 64)] * 1024L);
            tmp[CK(v * 8 + x, 64)] = sum;
        }
    for (y = 0; y < 8; ++y)
        for (x = 0; x < 8; ++x) {
            sum = 131584L;
            for (v = 0; v < 8; ++v)
                sum += tmul(cosine[CK(y * 8 + v, 64)], tmp[CK(v * 8 + x, 64)]);
            value = (int)(sum / 1024L);
            planes[CK(p * 4096 + (y0 + y) * 256 + x0 + x, 12288)] = (unsigned char)(value < 0 ? 0 : value > 255 ? 255 : value);
        }
}

static int clamp(long value)
{
    return value < 0 ? 0 : value > 255 ? 255 : (int)value;
}

/* One line of the MCU row as BGR, at (x >> hshift, y >> vshift) in the chroma planes; the constants are Q10. */
static void convert(int y, int width, int hshift, int vshift)
{
    int x;
    for (x = 0; x < width; ++x) {
        long luma = (long)planes[CK(y * 256 + x, 12288)] * 1024L;
        long blue = ((long)planes[CK(4096 + (y >> vshift) * 256 + (x >> hshift), 12288)] - 128L) * 1024L;
        long red = ((long)planes[CK(8192 + (y >> vshift) * 256 + (x >> hshift), 12288)] - 128L) * 1024L;
        line[CK(3 * x, 771)] = (unsigned char)clamp((luma + tmul(1815L, blue) + 512L) / 1024L);
        line[CK(3 * x + 1, 771)] = (unsigned char)clamp((luma - tmul(352L, blue) - tmul(731L, red) + 512L) / 1024L);
        line[CK(3 * x + 2, 771)] = (unsigned char)clamp((luma + tmul(1436L, red) + 512L) / 1024L);
    }
}

long bench_jpeg_fixed(void)
{
    int id[3] = {0}, sample[3] = {0}, qsel[3] = {0}, dcsel[3] = {0}, acsel[3] = {0}, pred[3] = {0};
    int width = 0, height = 0, interval = 0, scanned = 0, comps = 0;
    int marker, length, i, k, c, t, hmax, vmax, mcus_x, mcus_y, mx, my, bx, by, bw, bh, stride, left, out;
    unsigned long size;

    infile = bench_open_read("PHOTO.JPG");
    if (infile < 0) {
        message("PHOTO.JPG not found\r\n");
        return 0;
    }
    if (next() != 0xFF || next() != 0xD8) {
        message("not a JPEG\r\n");
        return 0;
    }
    while (!scanned && !bad) {
        while (next() != 0xFF && !bad)
            ;
        while ((marker = next()) == 0xFF)
            ;
        length = next16() - 2;
        if (marker == 0xDB) {
            while (length > 0) {
                t = next();
                if (t >> 4) {
                    message("16-bit quantization tables not supported\r\n");
                    return 0;
                }
                for (k = 0; k < 64; ++k)
                    quant[CK((t & 3) * 64 + k, 256)] = next();
                length -= 65;
            }
        } else if (marker == 0xC4) {
            while (length > 0 && !bad) {
                int counts[17] = {0}, code = 0, at = 0, n = 0;
                t = next();
                t = (t >> 4) * 2 + (t & 1);
                for (k = 1; k <= 16; ++k)
                    n += counts[CK(k, 17)] = next();
                for (k = 1; k <= 16; ++k) {
                    valptr[CK(t * 18 + k, 72)] = at;
                    mincode[CK(t * 18 + k, 72)] = code;
                    code += counts[CK(k, 17)];
                    at += counts[CK(k, 17)];
                    maxcode[CK(t * 18 + k, 72)] = counts[CK(k, 17)] ? code - 1 : -1;
                    code *= 2;
                }
                for (k = 0; k < n; ++k)
                    huffval[CK(t * 256 + k, 1024)] = (unsigned char)next();
                length -= 17 + n;
            }
        } else if (marker == 0xC0) {
            if (next() != 8) {
                message("only 8-bit samples are supported\r\n");
                return 0;
            }
            height = next16();
            width = next16();
            comps = next();
            for (i = 0; i < comps && i < 3; ++i) {
                id[CK(i, 3)] = next();
                sample[CK(i, 3)] = next();
                qsel[CK(i, 3)] = next() & 3;
            }
            length -= 6 + 3 * (comps < 3 ? comps : 3);
        } else if (marker == 0xC2) {
            message("progressive JPEG not supported\r\n");
            return 0;
        } else if (marker >= 0xC9 && marker <= 0xCF && marker != 0xCC) {
            message("arithmetic-coded JPEG not supported\r\n");
            return 0;
        } else if (marker >= 0xC1 && marker <= 0xCF && marker != 0xC4 && marker != 0xC8 && marker != 0xCC) {
            message("only baseline JPEG is supported\r\n");
            return 0;
        } else if (marker == 0xDD) {
            interval = next16();
            length -= 2;
        } else if (marker == 0xDA) {
            int n = next();
            for (i = 0; i < n; ++i) {
                int want = next(), selector = next();
                for (c = 0; c < 3; ++c)
                    if (id[CK(c, 3)] == want) {
                        dcsel[CK(c, 3)] = selector >> 4;
                        acsel[CK(c, 3)] = 2 + (selector & 1);
                    }
            }
            next();
            next();
            next();
            scanned = 1;
            length = 0;
        }
        while (length-- > 0)
            next();
    }
    hmax = sample[CK(0, 3)] >> 4;
    vmax = sample[CK(0, 3)] & 15;
    if (bad || comps != 3 || hmax < 1 || hmax > 2 || vmax < 1 || vmax > 2 || sample[CK(1, 3)] != 0x11 || sample[CK(2, 3)] != 0x11 || width < 1 || width > MAXW || height < 1) {
        message("unsupported JPEG\r\n");
        return 0;
    }
    bw = 8 * hmax;
    bh = 8 * vmax;
    mcus_x = (width + bw - 1) / bw;
    mcus_y = (height + bh - 1) / bh;
    stride = (width * 3 + 3) & ~3;
    size = 54UL + (unsigned long)stride * (unsigned long)height;
    for (i = 0; i < 54; ++i)
        line[CK(i, 771)] = 0;
    line[CK(0, 771)] = 'B';
    line[CK(1, 771)] = 'M';
    put32(2, size);
    put32(10, 54UL);
    put32(14, 40UL);
    put32(18, (unsigned long)width);
    put32(22, (unsigned long)-(long)height);
    line[CK(26, 771)] = 1;
    line[CK(28, 771)] = 24;
    put32(34, size - 54UL);
    put32(38, 2835UL);
    put32(42, 2835UL);
    out = bench_create("OUT.BMP");
    if (out < 0) {
        message("cannot write OUT.BMP\r\n");
        return 0;
    }
    emit(out, line, 54);
    for (i = 0; i < 3; ++i)
        pred[CK(i, 3)] = 0;
    left = interval;
    for (my = 0; my < mcus_y && !bad; ++my) {
        for (mx = 0; mx < mcus_x && !bad; ++mx) {
            if (interval && left == 0) {
                restart();
                for (i = 0; i < 3; ++i)
                    pred[CK(i, 3)] = 0;
                left = interval;
            }
            --left;
            for (c = 0; c < 3; ++c) {
                int h = c ? 1 : hmax, v = c ? 1 : vmax;
                for (by = 0; by < v; ++by)
                    for (bx = 0; bx < h; ++bx) {
                        for (k = 0; k < 64; ++k)
                            coef[CK(k, 64)] = 0;
                        t = decode(dcsel[CK(c, 3)]);
                        if (t)
                            pred[CK(c, 3)] += extend(receive(t), t);
                        coef[CK(0, 64)] = (long)pred[CK(c, 3)] * quant[CK(qsel[CK(c, 3)] * 64, 256)];
                        k = 1;
                        while (k < 64 && !bad) {
                            int rs = decode(acsel[CK(c, 3)]), run = rs >> 4, size_ = rs & 15;
                            if (size_ == 0) {
                                if (run != 15)
                                    break;
                                k += 16;
                                continue;
                            }
                            k += run;
                            if (k > 63)
                                break;
                            coef[CK(zigzag[CK(k, 64)], 64)] = (long)extend(receive(size_), size_) * quant[CK(qsel[CK(c, 3)] * 64 + k, 256)];
                            ++k;
                        }
                        idct(c, (mx * h + bx) * 8, by * 8);
                    }
            }
        }
        for (i = 0; i < bh && my * bh + i < height; ++i) {
            convert(i, width, hmax - 1, vmax - 1);
            for (k = width * 3; k < stride; ++k)
                line[CK(k, 771)] = 0;
            emit(out, line, stride);
        }
    }
    bench_close(out);
    bench_close(infile);
    return (long)((adler_b << 16) | adler_a);
}

void ebnd(void) {}

int main(void)
{
    report(bench_jpeg_fixed());
    return 0;
}
