// flags: -O2 -march=i486 -m32 | -O2 -march=i486
/* A switch of dense cases where one target takes most of them (a run of 22 values), so the dominant range is peeled off
   and the rest is a jump table: every value reaches the right target. */
extern void report(long value);

long pick(int marker, int fall)
{
    switch (marker) {
    case 0xC0: return 3;
    case 0xC2: return 4;
    case 0xC4: return 2;
    case 0xDB: return 1;
    case 0xC1: case 0xC3: case 0xC5: case 0xC6: case 0xC7: case 0xC8: case 0xC9: case 0xCA: case 0xCB: case 0xCC:
    case 0xCD: case 0xCE: case 0xCF: case 0xD0: case 0xD1: case 0xD2: case 0xD3: case 0xD4: case 0xD5: case 0xD6:
    case 0xD7: case 0xD8: case 0xD9: case 0xDA:
        return fall;
    default: return 0;
    }
}

long volatile start = 0;

int main(void)
{
    long sum = 0;
    int m;
    for (m = start; m < 256; ++m) {
        long v = pick(m, 9);
        if (v) report(m * 16 + v);
    }
    return 0;
}
