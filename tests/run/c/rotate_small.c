// flags: -O0 | -O2 | -Os
// A byte or word rotated by a count the program computes, or by a constant. LLVM writes it as a funnel shift of one value
// twice, which a target with no rotate instruction expands to shifts: at -O2 both widths were refused (CannotSelect Rotl).
extern void report(long value);

static unsigned char rotl8(unsigned char x, unsigned n) { return (unsigned char)((x << (n & 7)) | (x >> ((0u - n) & 7))); }
static unsigned char rotr8(unsigned char x, unsigned n) { return (unsigned char)((x >> (n & 7)) | (x << ((0u - n) & 7))); }
static unsigned short rotl16(unsigned short x, unsigned n) { return (unsigned short)((x << (n & 15)) | (x >> ((0u - n) & 15))); }
static unsigned short rotr16(unsigned short x, unsigned n) { return (unsigned short)((x >> (n & 15)) | (x << ((0u - n) & 15))); }

static const unsigned char bytes[3] = { 0x01, 0xA5, 0xFF };
static const unsigned short words[3] = { 0x0001, 0x1234, 0x8001 };

int main(void)
{
    unsigned long b = 0, w = 0, c = 0;
    unsigned i, n;
    for (i = 0; i < 3; ++i) {
        for (n = 0; n < 16; ++n) {
            b = b * 31UL + rotl8(bytes[i], n) + 3UL * rotr8(bytes[i], n);
            w = w * 31UL + rotl16(words[i], n) + 3UL * rotr16(words[i], n);
        }
        c = c * 31UL + rotl8(bytes[i], 3) + rotr8(bytes[i], 1) + 5UL * rotl16(words[i], 5) + 7UL * rotr16(words[i], 9);
    }
    /* halves: the sums wrap at the width of long, which differs between targets */
    report((long)(b & 0xFFFFUL));
    report((long)((b >> 16) & 0xFFFFUL));
    report((long)(w & 0xFFFFUL));
    report((long)((w >> 16) & 0xFFFFUL));
    report((long)(c & 0xFFFFUL));
    report((long)((c >> 16) & 0xFFFFUL));
    return 0;
}
