// flags: -O2 -march=i486 | -O2 -march=i486 -m32
// data: ../jpeg_single/photo.jpg
/* JPEG to BMP, an MCU row at a time: baseline Huffman YCbCr in 4:2:0, 4:4:4, 4:2:2 or 4:4:0, with restart markers. Reads
   PHOTO.JPG, writes a top-down 24-bit OUT.BMP and returns the Adler-32 of the BMP's bytes. The IDCT and the colour
   conversion are in 32-bit fixed point (long; >> floors); chroma is replicated. A progressive or arithmetic-coded file is refused with a message. */
#include "../bench_io.h"
extern void report(long value);

#define MAXW 256 /* the widest picture: a row of MCUs is buffered */

/* cos((2x+1)u*pi/16) * (u ? 1 : 1/sqrt(2)) / 2 in Q12 */
static const int cosine[8][8] = {
    {1448, 2009, 1892, 1703, 1448, 1138, 784, 400},
    {1448, 1703, 784, -400, -1448, -2009, -1892, -1138},
    {1448, 1138, -784, -2009, -1448, 400, 1892, 1703},
    {1448, 400, -1892, -1138, 1448, 1703, -784, -2009},
    {1448, -400, -1892, 1138, 1448, -1703, -784, 2009},
    {1448, -1138, -784, 2009, -1448, -400, 1892, -1703},
    {1448, -1703, 784, 400, -1448, 2009, -1892, 1138},
    {1448, -2009, 1892, -1703, 1448, -1138, 784, -400}};
static const unsigned char zigzag[64] = {0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28,
                                         35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63};

static int infile, bad;
static unsigned char inbuf[512], planes[3][16][MAXW], line[MAXW * 3 + 3], bmp[54];
static int inpos, inlen, bitbuf, bitcnt;
static int quant[4][64], maxcode[4][18], valptr[4][17], mincode[4][17];
static unsigned char huffval[4][256];
static long coef[64];
static unsigned long adler_a = 1, adler_b;

static int next(void)
{
    if (inpos == inlen) {
        inlen = bench_read(infile, inbuf, sizeof inbuf);
        inpos = 0;
        if (inlen <= 0) {
            inlen = 0;
            bad = 1;
            return 0;
        }
    }
    return inbuf[inpos++];
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
    while (code > maxcode[table][length]) {
        code = code * 2 + getbit();
        if (++length > 16) {
            bad = 1;
            return 0;
        }
    }
    return huffval[table][valptr[table][length] + code - mincode[table][length]];
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
        adler_a += data[i];
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
        bmp[at + i] = (unsigned char)(value >> (8 * i));
}

static void message(const char *text)
{
    int length = 0;
    while (text[length])
        ++length;
    bench_write(1, text, length);
}

/* The 8x8 block in coef, into plane `p` at (x0, y0): rows to Q2 (Q12 times the coefficient, >> 10), then columns to
   integers, +128. */
static void idct(int p, int x0, int y0)
{
    long tmp[64], sum;
    int v, x, u, y, value;

    for (v = 0; v < 8; ++v)
        for (x = 0; x < 8; ++x) {
            sum = 512L;
            for (u = 0; u < 8; ++u)
                sum += cosine[x][u] * coef[v * 8 + u];
            tmp[v * 8 + x] = sum >> 10;
        }
    for (y = 0; y < 8; ++y)
        for (x = 0; x < 8; ++x) {
            sum = 8192L;
            for (v = 0; v < 8; ++v)
                sum += cosine[y][v] * tmp[v * 8 + x];
            value = (int)(sum >> 14) + 128;
            planes[p][y0 + y][x0 + x] = (unsigned char)(value < 0 ? 0 : value > 255 ? 255 : value);
        }
}

static int clamp(long value)
{
    return value < 0 ? 0 : value > 255 ? 255 : (int)value;
}

/* One line of the MCU row as BGR, at (x >> hshift, y >> vshift) in the chroma planes; the constants are Q16. */
static void convert(int y, int width, int hshift, int vshift)
{
    int x;
    for (x = 0; x < width; ++x) {
        long luma = planes[0][y][x];
        long blue = (long)planes[1][y >> vshift][x >> hshift] - 128L;
        long red = (long)planes[2][y >> vshift][x >> hshift] - 128L;
        line[3 * x] = (unsigned char)clamp(luma + ((116130L * blue + 32768L) >> 16));
        line[3 * x + 1] = (unsigned char)clamp(luma + ((-22554L * blue - 46802L * red + 32768L) >> 16));
        line[3 * x + 2] = (unsigned char)clamp(luma + ((91881L * red + 32768L) >> 16));
    }
}

long bench_jpeg_fixed(void)
{
    int id[3], sample[3], qsel[3], dcsel[3], acsel[3], pred[3];
    int width = 0, height = 0, interval = 0, scanned = 0, comps = 0;
    int marker, length, i, k, c, t, hmax, vmax, mcus_x, mcus_y, mx, my, bx, by, bw, bh, stride, left, out;
    unsigned long size;

    infile = bench_open_read("PHOTO.JPG");
    if (infile < 0) {
        message("PHOTO.JPG not found\r\n");
        return 0;
    }
    if (next16() != 0xFFD8) {
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
                    quant[t & 3][k] = next();
                length -= 65;
            }
        } else if (marker == 0xC4) {
            while (length > 0 && !bad) {
                int counts[17], code = 0, at = 0, n = 0;
                t = next();
                t = (t >> 4) * 2 + (t & 1);
                for (k = 1; k <= 16; ++k)
                    n += counts[k] = next();
                for (k = 1; k <= 16; ++k) {
                    valptr[t][k] = at;
                    mincode[t][k] = code;
                    code += counts[k];
                    at += counts[k];
                    maxcode[t][k] = counts[k] ? code - 1 : -1;
                    code *= 2;
                }
                for (k = 0; k < n; ++k)
                    huffval[t][k] = (unsigned char)next();
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
                id[i] = next();
                sample[i] = next();
                qsel[i] = next() & 3;
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
                    if (id[c] == want) {
                        dcsel[c] = selector >> 4;
                        acsel[c] = 2 + (selector & 1);
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
    hmax = sample[0] >> 4;
    vmax = sample[0] & 15;
    if (bad || comps != 3 || hmax < 1 || hmax > 2 || vmax < 1 || vmax > 2 || sample[1] != 0x11 || sample[2] != 0x11 || width < 1 || width > MAXW || height < 1) {
        message("unsupported JPEG\r\n");
        return 0;
    }
    bw = 8 * hmax;
    bh = 8 * vmax;
    mcus_x = (width + bw - 1) / bw;
    mcus_y = (height + bh - 1) / bh;
    stride = (width * 3 + 3) & ~3;
    size = 54UL + (unsigned long)stride * (unsigned long)height;
    bmp[0] = 'B';
    bmp[1] = 'M';
    put32(2, size);
    put32(10, 54UL);
    put32(14, 40UL);
    put32(18, (unsigned long)width);
    put32(22, (unsigned long)-(long)height);
    bmp[26] = 1;
    bmp[28] = 24;
    put32(34, size - 54UL);
    put32(38, 2835UL);
    put32(42, 2835UL);
    out = bench_create("OUT.BMP");
    if (out < 0) {
        message("cannot write OUT.BMP\r\n");
        return 0;
    }
    emit(out, bmp, 54);
    for (i = 0; i < 3; ++i)
        pred[i] = 0;
    left = interval;
    for (my = 0; my < mcus_y && !bad; ++my) {
        for (mx = 0; mx < mcus_x && !bad; ++mx) {
            if (interval && left == 0) {
                restart();
                for (i = 0; i < 3; ++i)
                    pred[i] = 0;
                left = interval;
            }
            --left;
            for (c = 0; c < 3; ++c) {
                int h = c ? 1 : hmax, v = c ? 1 : vmax;
                for (by = 0; by < v; ++by)
                    for (bx = 0; bx < h; ++bx) {
                        for (k = 0; k < 64; ++k)
                            coef[k] = 0;
                        t = decode(dcsel[c]);
                        if (t)
                            pred[c] += extend(receive(t), t);
                        coef[0] = (long)pred[c] * quant[qsel[c]][0];
                        k = 1;
                        while (k < 64 && !bad) {
                            int rs = decode(acsel[c]), run = rs >> 4, size_ = rs & 15;
                            if (size_ == 0) {
                                if (run != 15)
                                    break;
                                k += 16;
                                continue;
                            }
                            k += run;
                            if (k > 63)
                                break;
                            coef[zigzag[k]] = (long)extend(receive(size_), size_) * quant[qsel[c]][k];
                            ++k;
                        }
                        idct(c, (mx * h + bx) * 8, by * 8);
                    }
            }
        }
        for (i = 0; i < bh && my * bh + i < height; ++i) {
            convert(i, width, hmax - 1, vmax - 1);
            for (k = width * 3; k < stride; ++k)
                line[k] = 0;
            emit(out, line, stride);
        }
    }
    bench_close(out);
    bench_close(infile);
    return (long)((adler_b << 16) | adler_a);
}

int main(void)
{
    report(bench_jpeg_fixed());
    return 0;
}
