int printf(const char *, ...);
static int a32(int x, int y) { return (x + y) * 3 - (x ^ y) + (x & 0xff) - (y | 7) + (x << 3) + (y >> 2); }
static unsigned u32(unsigned x, unsigned y) { return (x * 7u) / (y | 1u) + (x % 13u) + (x >> 5) + (y << 2); }
static long a64(long x, long y) { return (x * y) / (y | 1) + (x % 101) - (x >> 3) + (y << 5) + (x & 0xffffffffL); }
static unsigned long u64(unsigned long x, unsigned long y) { return x * y + (x / (y | 1)) + (x >> 7) + (y << 11) + (x ^ y); }
static char c8(char x, char y) { return (char)(x + y) * 3; }
static unsigned char uc8(unsigned char x, unsigned char y) { return (unsigned char)(x * y + 17); }
static short s16(short x, short y) { return (short)(x * y - 100) >> 1; }
static unsigned short us16(unsigned short x, unsigned short y) { return (unsigned short)(x * y + 3) >> 2; }
int main(void) {
  for (int i = -3; i < 4; i++) {
    for (int j = -2; j < 3; j++) {
      printf("%d %d %d ", a32(i * 1000, j * 77), (int)u32((unsigned)i * 999u, (unsigned)j), (int)a64(i * 100000000L, j * 3));
      printf("%lu %d %d %d\n", u64((unsigned long)i * 123456789ul, (unsigned long)(j + 5)), c8((char)i, (char)j), uc8((unsigned char)(i * 50), (unsigned char)(j * 30)), s16((short)(i * 1000), (short)(j * 200)));
      printf("%d\n", us16((unsigned short)(i * 4000), (unsigned short)(j * 5000)));
    }
  }
  return 0;
}
