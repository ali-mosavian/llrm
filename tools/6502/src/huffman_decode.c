// flags: -O2 -march=i486 -fno-inline-functions -fno-inline-functions-called-once | -O2 -march=i486 -m32 -fno-inline-functions -fno-inline-functions-called-once
/* A canonical Huffman decode over two-dimensional tables (JPEG's): code lengths, `maxcode`, `mincode`, `valptr` and
   the value table, indexed by the table number and the length. */
extern void report(long value);

static int maxcode[4][18], valptr[4][17], mincode[4][17];
static unsigned char huffval[4][256];
static unsigned char stream[16];
static int bitpos;
static int bad;

static int getbit(void)
{
    int bit = (stream[bitpos >> 3] >> (7 - (bitpos & 7))) & 1;
    ++bitpos;
    return bit;
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

static void build(int t, const int *counts, int n)
{
    int k, code = 0, at = 0;
    for (k = 1; k <= 16; ++k) {
        valptr[t][k] = at;
        mincode[t][k] = code;
        code += counts[k];
        at += counts[k];
        maxcode[t][k] = counts[k] ? code - 1 : -1;
        code *= 2;
    }
    for (k = 0; k < n; ++k)
        huffval[t][k] = (unsigned char)(10 + 3 * k + t);
}

long volatile zero = 0;

int main(void)
{
    int counts[17], t, i, n;
    for (i = 0; i < 17; ++i) counts[i] = 0;
    counts[2] = 2; counts[3] = 3; counts[5] = 1;
    n = 6;
    for (t = 0; t < 4; ++t) build(t, counts, n);
    for (i = 0; i < 16; ++i) stream[i] = (unsigned char)(0xB3 * (i + 1) + 0x55 + zero);
    for (t = 0; t < 4; ++t) {
        bitpos = 0;
        for (i = 0; i < 12 && !bad; ++i) report(decode(t) * 10 + t);
    }
    report(bad);
    return 0;
}
