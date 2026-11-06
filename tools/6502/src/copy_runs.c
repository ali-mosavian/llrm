// flags: -O0 | -Os | -Os -m32 | -O3
// Straight-line loads and stores that copy adjacent cells are one memcpy where the two sides are apart (gcc
// leaves them; LLVM's MemCpyOpt). A copy of a table onto itself one cell on is a smear, not a memcpy:
// each answer must be that of the pairs in order.
extern void report(long value);

static short s[48], d[48], t[49];
static signed char v[48];
static char w[48];

static void copy(void)
{
    d[0] = s[0];
    d[1] = s[1];
    d[2] = s[2];
    d[3] = s[3];
    d[4] = s[4];
    d[5] = s[5];
    d[6] = s[6];
    d[7] = s[7];
    d[8] = s[8];
    d[9] = s[9];
    d[10] = s[10];
    d[11] = s[11];
    d[12] = s[12];
    d[13] = s[13];
    d[14] = s[14];
    d[15] = s[15];
    d[16] = s[16];
    d[17] = s[17];
    d[18] = s[18];
    d[19] = s[19];
    d[20] = s[20];
    d[21] = s[21];
    d[22] = s[22];
    d[23] = s[23];
    d[24] = s[24];
    d[25] = s[25];
    d[26] = s[26];
    d[27] = s[27];
    d[28] = s[28];
    d[29] = s[29];
    d[30] = s[30];
    d[31] = s[31];
    d[32] = s[32];
    d[33] = s[33];
    d[34] = s[34];
    d[35] = s[35];
    d[36] = s[36];
    d[37] = s[37];
    d[38] = s[38];
    d[39] = s[39];
    d[40] = s[40];
    d[41] = s[41];
    d[42] = s[42];
    d[43] = s[43];
    d[44] = s[44];
    d[45] = s[45];
    d[46] = s[46];
    d[47] = s[47];
}

static void smear(void)
{
    t[1] = t[0];
    t[2] = t[1];
    t[3] = t[2];
    t[4] = t[3];
    t[5] = t[4];
    t[6] = t[5];
    t[7] = t[6];
    t[8] = t[7];
    t[9] = t[8];
    t[10] = t[9];
    t[11] = t[10];
    t[12] = t[11];
    t[13] = t[12];
    t[14] = t[13];
    t[15] = t[14];
    t[16] = t[15];
    t[17] = t[16];
    t[18] = t[17];
    t[19] = t[18];
    t[20] = t[19];
    t[21] = t[20];
    t[22] = t[21];
    t[23] = t[22];
    t[24] = t[23];
    t[25] = t[24];
    t[26] = t[25];
    t[27] = t[26];
    t[28] = t[27];
    t[29] = t[28];
    t[30] = t[29];
    t[31] = t[30];
    t[32] = t[31];
    t[33] = t[32];
    t[34] = t[33];
    t[35] = t[34];
    t[36] = t[35];
    t[37] = t[36];
    t[38] = t[37];
    t[39] = t[38];
    t[40] = t[39];
    t[41] = t[40];
    t[42] = t[41];
    t[43] = t[42];
    t[44] = t[43];
    t[45] = t[44];
    t[46] = t[45];
    t[47] = t[46];
    t[48] = t[47];
}

static void narrow(void)
{
    w[0] = (char)v[0];
    w[1] = (char)v[1];
    w[2] = (char)v[2];
    w[3] = (char)v[3];
    w[4] = (char)v[4];
    w[5] = (char)v[5];
    w[6] = (char)v[6];
    w[7] = (char)v[7];
    w[8] = (char)v[8];
    w[9] = (char)v[9];
    w[10] = (char)v[10];
    w[11] = (char)v[11];
    w[12] = (char)v[12];
    w[13] = (char)v[13];
    w[14] = (char)v[14];
    w[15] = (char)v[15];
    w[16] = (char)v[16];
    w[17] = (char)v[17];
    w[18] = (char)v[18];
    w[19] = (char)v[19];
    w[20] = (char)v[20];
    w[21] = (char)v[21];
    w[22] = (char)v[22];
    w[23] = (char)v[23];
    w[24] = (char)v[24];
    w[25] = (char)v[25];
    w[26] = (char)v[26];
    w[27] = (char)v[27];
    w[28] = (char)v[28];
    w[29] = (char)v[29];
    w[30] = (char)v[30];
    w[31] = (char)v[31];
    w[32] = (char)v[32];
    w[33] = (char)v[33];
    w[34] = (char)v[34];
    w[35] = (char)v[35];
    w[36] = (char)v[36];
    w[37] = (char)v[37];
    w[38] = (char)v[38];
    w[39] = (char)v[39];
    w[40] = (char)v[40];
    w[41] = (char)v[41];
    w[42] = (char)v[42];
    w[43] = (char)v[43];
    w[44] = (char)v[44];
    w[45] = (char)v[45];
    w[46] = (char)v[46];
    w[47] = (char)v[47];
}

static long sum(const short *p, int n)
{
    long total = 0;
    int i;
    for (i = 0; i < n; ++i)
        total = (total * 3 + p[i]) % 1000003;
    return total;
}

int main(void)
{
    int i;
    for (i = 0; i < 48; ++i)
    {
        s[i] = (short)(i * 7 + 1);
        t[i] = (short)(i * 5 + 2);
        v[i] = (signed char)(i - 20);
    }
    copy();
    smear();
    narrow();
    report(sum(d, 48));
    report(sum(t, 49));
    report(w[0] + w[47] + w[17]);
    return 0;
}
