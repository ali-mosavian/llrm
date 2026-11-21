#define N 1200
static unsigned char a8[N], b8[N];

static int ne8(void) {
    for (int i = 0; i < N; i++)
        if (a8[i] != b8[i])
            return 1;
    return 0;
}

int differs_from_expected(int p) { return ne8() != (p >= 0); }

int touch(int p) {
    a8[p] ^= 1;
    b8[p] = 1;
    return b8[p];
}
