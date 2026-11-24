/* A library: five entries, each run once and none reached from a `main`.
 * A `static inline` callee (clang's `inlinehint`) of about twenty instructions,
 * called from six places: inlined everywhere it is six copies, out of line one
 * body and six calls (gcc -Os keeps the one). */
typedef unsigned int u32;

static inline int lead_zeros(u32 x)
{
    int n = 0;
    if (!(x & 0xFFFF0000u)) { n += 16; x <<= 16; }
    if (!(x & 0xFF000000u)) { n += 8; x <<= 8; }
    if (!(x & 0xF0000000u)) { n += 4; x <<= 4; }
    if (!(x & 0xC0000000u)) { n += 2; x <<= 2; }
    if (!(x & 0x80000000u)) { n += 1; }
    return n;
}

int norm_a(u32 m) { return lead_zeros(m) - 8; }
int norm_b(u32 m) { return lead_zeros(m) - 5; }
int norm_c(u32 m, u32 n) { return lead_zeros(m) + lead_zeros(n); }
int norm_d(u32 m) { return 31 - lead_zeros(m); }
int norm_e(u32 m, u32 n) { return lead_zeros(m & n) * 3; }
