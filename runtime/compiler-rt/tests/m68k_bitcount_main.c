/* Counts of non-zero values through __clzsi2 __ctzsi2 __clzdi2 __ctzdi2 against loops, no 64-bit operation (they would be libcalls). Exit 0 when every count is right,
 * else the number of wrong ones. Linked by tests/m68k_bitcount_bfffo.sh with the routines under test; _start exits by trap #0. */
int __clzsi2(unsigned x);
int __ctzsi2(unsigned x);
int __clzdi2(unsigned long long x);
int __ctzdi2(unsigned long long x);

static int clz32(unsigned x) { int n = 0; while (!(x & 0x80000000u)) { x <<= 1; n++; } return n; }
static int ctz32(unsigned x) { int n = 0; while (!(x & 1u)) { x >>= 1; n++; } return n; }

union wide { unsigned long long all; struct { unsigned hi, lo; } part; };

static int bad;
static void word(unsigned x)
{
    if (!x) return;
    bad += __clzsi2(x) != clz32(x);
    bad += __ctzsi2(x) != ctz32(x);
}
static void pair(unsigned hi, unsigned lo)
{
    union wide v;
    v.part.hi = hi;
    v.part.lo = lo;
    if (!hi && !lo) return;
    bad += __clzdi2(v.all) != (hi ? clz32(hi) : 32 + clz32(lo));
    bad += __ctzdi2(v.all) != (lo ? ctz32(lo) : 32 + ctz32(hi));
}

int main(void)
{
    static const unsigned edge[] = {0x12345678u, 0x80000001u, 0xFFFFFFFFu, 0x00010000u, 0xA5A5A5A5u, 0x7FFFFFFFu};
    for (int i = 0; i < 32; i++) {
        unsigned bit = 1u << i, up = 0xFFFFFFFFu << i, down = 0xFFFFFFFFu >> i;
        word(bit); word(up); word(down); word(~bit);
        pair(bit, 0); pair(0, bit); pair(up, 0); pair(0, up); pair(down, 0xFFFFFFFFu); pair(0xFFFFFFFFu, up);
    }
    for (int i = 0; i < 6; i++) {
        word(edge[i]);
        for (int j = 0; j < 6; j++) pair(edge[i], edge[j]);
    }
    return bad;
}

__asm__(".globl _start\n_start:\n\tjsr main\n\tmove.l %d0,%d1\n\tmoveq #1,%d0\n\ttrap #0\n");
