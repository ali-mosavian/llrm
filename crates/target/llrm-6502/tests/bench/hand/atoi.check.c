/* hand atoi against k_atoi: signs, white space (space and tab only, as in cc65),
   overflow (low 16 bits), junk, high-bit bytes, and strings whose offset passes 256. */
int __fastcall__ atoi(const char *s);
int k_atoi(const char *s);

extern char num[];

static char big[600];

static int cmp(const char *s)
{
    return atoi(s) != k_atoi(s);
}

int main(void)
{
    unsigned i;
    if (cmp("")) return 1;
    if (cmp("0")) return 2;
    if (cmp("42")) return 3;
    if (cmp(num)) return 4;
    if (cmp("+17")) return 5;
    if (cmp("-")) return 6;
    if (cmp("+")) return 7;
    if (cmp("\t 7")) return 8;
    if (cmp("\n7")) return 9;
    if (cmp("32767")) return 10;
    if (cmp("32768")) return 11;
    if (cmp("-32768")) return 12;
    if (cmp("65535")) return 13;
    if (cmp("65536")) return 14;
    if (cmp("99999")) return 15;
    if (cmp("-99999")) return 16;
    if (cmp("12345678901234567890")) return 17;
    if (cmp("007x")) return 18;
    if (cmp("--5")) return 19;
    if (cmp("+-5")) return 20;
    if (cmp("12abc")) return 21;
    if (cmp("\xE9" "5")) return 22;
    if (cmp("5\xE9")) return 23;
    if (cmp(" - 5")) return 24;
    for (i = 0; i < 300; ++i)
        big[i] = ' ';
    big[300] = '-';
    big[301] = '9';
    big[302] = '1';
    if (cmp(big)) return 25;
    for (i = 0; i < 300; ++i)
        big[i] = (char)('1' + i % 9);
    big[300] = 0;
    if (cmp(big)) return 26;
    big[259] = 0;
    if (cmp(big)) return 27;
    big[255] = 0;
    if (cmp(big)) return 28;
    return 0;
}
