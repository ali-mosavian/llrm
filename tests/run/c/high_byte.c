// flags: -O2 -march=i486
// targets: x86-m16
/* The high byte of a word, taken and extended: the pin's EXTRACT_SUBREG of AH and MOVZX32rr8_NOREX were refused
   (13 and 9 QCport functions at m16). */
extern void report(long value);

static unsigned char __attribute__((noinline)) high(unsigned short x) { return (unsigned char)(x >> 8); }
static unsigned long __attribute__((noinline)) widen(unsigned short x) { return (unsigned long)(unsigned char)(x >> 8) * 3ul; }
static long __attribute__((noinline)) signed_high(short x) { return (signed char)(x >> 8); }

int main(void)
{
    report(high(0x1234));
    report(high(0xFF01));
    report((long)widen(0xAB00));
    report(signed_high((short)0x8001));
    report(signed_high(0x7F00));
    return 0;
}
