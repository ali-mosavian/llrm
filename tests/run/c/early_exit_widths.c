/* A loop that leaves at the first lane that differs, at every lane width and compare it vectorizes: the
   any-of of the vector compare is a byte mask (`pmovmskb`) on x86. The difference sits at the first and last
   lane of a vector, in a trip of the scalar remainder, and nowhere. */
extern void report(long value);

#define N 1200
static unsigned char a8[N], b8[N];
static unsigned short a16[N], b16[N];
static int a32[N], b32[N];
static signed char s8[N], t8[N];

static int ne8(void) { for (int i = 0; i < N; i++) if (a8[i] != b8[i]) return 1; return 0; }
static int lt16(void) { for (int i = 0; i < N; i++) if (a16[i] < b16[i]) return 1; return 0; }
static int ne32(void) { for (int i = 0; i < N; i++) if (a32[i] != b32[i]) return 1; return 0; }
static int gt8(void) { for (int i = 0; i < N; i++) if (s8[i] > t8[i]) return 1; return 0; }

static void reset(void) {
    for (int i = 0; i < N; i++) {
        a8[i] = b8[i] = (unsigned char)(i * 7);
        a16[i] = 40000; b16[i] = 40000;
        a32[i] = b32[i] = i * 31;
        s8[i] = t8[i] = (signed char)(i * 5);
    }
}

int main(void) {
    static const int at[] = {0, 1, 7, 8, 15, 16, 17, 31, 32, 599, 1183, 1184, 1199, -1};
    int bad = 0;
    for (unsigned k = 0; k < sizeof at / sizeof at[0]; k++) {
        int p = at[k], want = p >= 0;
        reset();
        if (p >= 0) {
            a8[p] ^= 0x80; a16[p] = 39999; a32[p] += 1; s8[p] = 127; t8[p] = -128;
        }
        bad += ne8() != want;
        bad += lt16() != want;
        bad += ne32() != want;
        bad += gt8() != want;
    }
    report(bad);
    return 0;
}
