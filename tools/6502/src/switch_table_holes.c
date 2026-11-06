// flags: -O1 | -O2 | -Os
// A guarded switch lookup table has poison holes: the backend refused them as "a poison initializer" (item.c after #3422).
extern void report(long value);

static char buf[256];

static int bump(int kind, int v)
{
    int off;
    switch (kind) {
    case 7: off = 120; break;
    case 8: off = 124; break;
    case 16: off = 128; break;
    case 20: off = 116; break;
    default: return 0;
    }
    buf[off] += v;
    return 1;
}

int main(void)
{
    int k;
    long sum = 0;
    for (k = 0; k < 24; k++)
        sum = sum * 31 + bump(k, k + 1);
    for (k = 0; k < 256; k++)
        sum = sum * 7 + buf[k];
    report(sum);
    return 0;
}
