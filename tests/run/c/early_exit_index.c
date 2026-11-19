/* A loop that returns the index of the first difference: the early edge takes the induction of the first set lane,
   cast to the return type (`return (int)i`, `(short)i`, `(long)i`). The difference sits at the first and last
   lane of a vector, in a trip of the scalar remainder, and nowhere. */
extern void report(long value);

#define N 1200
static unsigned char a[N], b[N];

static int index32(void) { for (int i = 0; i < N; i++) if (a[i] != b[i]) return i; return -1; }
static long index64(void) { for (long i = 0; i < N; i++) if (a[i] != b[i]) return i; return -1; }
static unsigned short index16(void) { for (int i = 0; i < N; i++) if (a[i] != b[i]) return (unsigned short)i; return 65535; }

int main(void) {
    static const int at[] = {0, 1, 7, 8, 15, 16, 17, 31, 32, 599, 1183, 1184, 1199, -1};
    int bad = 0;
    for (unsigned k = 0; k < sizeof at / sizeof at[0]; k++) {
        int p = at[k];
        for (int i = 0; i < N; i++) a[i] = b[i] = (unsigned char)(i * 7);
        if (p >= 0) a[p] ^= 0x80;
        bad += index32() != p;
        bad += index64() != p;
        bad += index16() != (p >= 0 ? p : 65535);
    }
    report(bad);
    return 0;
}
