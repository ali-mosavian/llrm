/* The builtins, not the library names: the SDK header turns those into `__memcpy_chk` calls, which the
   selector never sees as an intrinsic.

   Constant-length memcpy, memmove and memset, expanded in place for lengths within budget, at every
   size around the access widths: each byte copied once, a memmove of overlapping ranges both ways, and
   no byte outside the range written. */
#include "llrm_os.h"

static unsigned char src[1200], dst[1200], ref[1200];
static int bad;

#define SIZES(X) X(1) X(2) X(3) X(5) X(7) X(8) X(9) X(15) X(16) X(17) X(24) X(31) X(32) X(33) X(40) X(63) X(64) \
    X(65) X(100) X(127) X(128) X(129) X(200) X(255) X(256) X(257) X(300) X(511) X(512) X(513) X(600)

/* Only the OS layer is linked, so no stdio and no string.h: the comparison and the report are done here. */
static void say(const char *text) {
    int n = 0;
    while (text[n]) n++;
    llrm_os_write_file(1, (const unsigned char *)text, n);
}
static void say_number(int value) {
    char digits[8];
    int n = 0;
    do digits[n++] = (char)('0' + value % 10); while ((value /= 10) != 0);
    while (n--) llrm_os_write_file(1, (const unsigned char *)&digits[n], 1);
}
static int differ(const unsigned char *a, const unsigned char *b, int n) {
    for (int i = 0; i < n; i++)
        if (a[i] != b[i]) return 1;
    return 0;
}

/* The reference moves bytes one at a time through volatile accesses, so no pass turns it into the call under test. */
__attribute__((noinline)) static void ref_copy(unsigned char *d, const unsigned char *s, int n) {
    volatile unsigned char *vd = d;
    for (int i = 0; i < n; i++) vd[i] = s[i];
}
__attribute__((noinline)) static void ref_fill(unsigned char *d, int v, int n) {
    volatile unsigned char *vd = d;
    for (int i = 0; i < n; i++) vd[i] = (unsigned char)v;
}
__attribute__((noinline)) static void ref_move(unsigned char *d, const unsigned char *s, int n) {
    unsigned char tmp[1200];
    ref_copy(tmp, s, n);
    ref_copy(d, tmp, n);
}

#define DEFINE(n) \
    __attribute__((noinline)) static void cp##n(unsigned char *d, const unsigned char *s) { __builtin_memcpy(d, s, n); } \
    __attribute__((noinline)) static void mv##n(unsigned char *d, const unsigned char *s) { __builtin_memmove(d, s, n); } \
    __attribute__((noinline)) static void zr##n(unsigned char *d) { __builtin_memset(d, 0, n); } \
    __attribute__((noinline)) static void c7##n(unsigned char *d) { __builtin_memset(d, 0x7b, n); } \
    __attribute__((noinline)) static void vr##n(unsigned char *d, int v) { __builtin_memset(d, v, n); }
SIZES(DEFINE)

/* The same on frame objects at odd offsets, where the address is a slot and not a global or a register. */
#define LOCAL(n) \
    __attribute__((noinline)) static void lc##n(unsigned char *out, const unsigned char *pattern) { \
        unsigned char a[700], b[700]; \
        for (int i = 0; i < 700; i++) a[i] = pattern[i], b[i] = 0xEE; \
        __builtin_memcpy(b + 3, a + 1, n); \
        __builtin_memmove(a + 5, a + 2, n < 600 ? n : 600); \
        __builtin_memset(b + 650, 0x33, n < 40 ? n : 40); \
        for (int i = 0; i < 700; i++) out[i] = (unsigned char)(a[i] ^ b[i]); \
    } \
    __attribute__((noinline)) static void lr##n(unsigned char *out, const unsigned char *pattern) { \
        unsigned char a[700], b[700]; \
        for (int i = 0; i < 700; i++) a[i] = pattern[i], b[i] = 0xEE; \
        ref_copy(b + 3, a + 1, n); \
        ref_move(a + 5, a + 2, n < 600 ? n : 600); \
        ref_fill(b + 650, 0x33, n < 40 ? n : 40); \
        for (int i = 0; i < 700; i++) out[i] = (unsigned char)(a[i] ^ b[i]); \
    }

static void check(const char *what, int n, int at) {
    if (differ(dst, ref, sizeof dst)) {
        say("bad "), say(what), say(" "), say_number(n), say(" at "), say_number(at), say("\r\n");
        bad = 1;
    }
}

static void reset(void) {
    for (int i = 0; i < 1200; i++)
        src[i] = (unsigned char)(i * 7 + 3), dst[i] = 0xEE;
    for (int i = 0; i < 1200; i++) ref[i] = dst[i];
}

#define RUN(n) \
    reset(); cp##n(dst + 5, src + 1); ref_copy(ref + 5, src + 1, n); check("copy", n, 0); \
    reset(); zr##n(dst + 3); ref_fill(ref + 3, 0, n); check("zero", n, 0); \
    reset(); c7##n(dst + 3); ref_fill(ref + 3, 0x7b, n); check("fill", n, 0); \
    reset(); vr##n(dst + 3, 0x5c1); ref_fill(ref + 3, 0xc1, n); check("fill var", n, 0); \
    reset(); ref_copy(dst, src, 1200); ref_copy(ref, src, 1200); \
    mv##n(dst + 4, dst + 1); ref_move(ref + 4, ref + 1, n); check("move up", n, 0); \
    reset(); ref_copy(dst, src, 1200); ref_copy(ref, src, 1200); \
    mv##n(dst + 1, dst + 4); ref_move(ref + 1, ref + 4, n); check("move down", n, 0);

SIZES(LOCAL)

static void check_locals(int n, void (*under)(unsigned char *, const unsigned char *),
                         void (*reference)(unsigned char *, const unsigned char *)) {
    static unsigned char x[700], y[700];
    under(x, src);
    reference(y, src);
    if (differ(x, y, sizeof x)) {
        say("bad locals "), say_number(n), say("\r\n");
        bad = 1;
    }
}

#define RUN_LOCAL(n) check_locals(n, lc##n, lr##n);

int main(void) {
    SIZES(RUN)
    SIZES(RUN_LOCAL)
    say(bad ? "FAIL\r\n" : "ok\r\n");
    return bad;
}
