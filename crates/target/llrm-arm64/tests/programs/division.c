int printf(const char *, ...);
typedef unsigned long u64;
static const signed char i8_values[] = {(signed char)0, (signed char)1, (signed char)(-1), (signed char)2, (signed char)(-2), (signed char)7, (signed char)(-7), (signed char)100, (signed char)(-100), (signed char)127, (signed char)(-128), (signed char)55, (signed char)(-55)};
static u64 i8_run(void) {
  u64 h = 7;
  for (unsigned i = 0; i < sizeof i8_values / sizeof i8_values[0]; i++) {
    signed char x = i8_values[i];
    h = h * 31 + (u64)(long)(x / (signed char)2); h = h * 31 + (u64)(long)(x % (signed char)2);
    h = h * 31 + (u64)(long)(x / (signed char)3); h = h * 31 + (u64)(long)(x % (signed char)3);
    h = h * 31 + (u64)(long)(x / (signed char)-3); h = h * 31 + (u64)(long)(x % (signed char)-3);
    h = h * 31 + (u64)(long)(x / (signed char)5); h = h * 31 + (u64)(long)(x % (signed char)5);
    h = h * 31 + (u64)(long)(x / (signed char)6); h = h * 31 + (u64)(long)(x % (signed char)6);
    h = h * 31 + (u64)(long)(x / (signed char)7); h = h * 31 + (u64)(long)(x % (signed char)7);
    h = h * 31 + (u64)(long)(x / (signed char)-7); h = h * 31 + (u64)(long)(x % (signed char)-7);
    h = h * 31 + (u64)(long)(x / (signed char)9); h = h * 31 + (u64)(long)(x % (signed char)9);
    h = h * 31 + (u64)(long)(x / (signed char)10); h = h * 31 + (u64)(long)(x % (signed char)10);
    h = h * 31 + (u64)(long)(x / (signed char)-10); h = h * 31 + (u64)(long)(x % (signed char)-10);
    h = h * 31 + (u64)(long)(x / (signed char)11); h = h * 31 + (u64)(long)(x % (signed char)11);
    h = h * 31 + (u64)(long)(x / (signed char)12); h = h * 31 + (u64)(long)(x % (signed char)12);
    h = h * 31 + (u64)(long)(x / (signed char)13); h = h * 31 + (u64)(long)(x % (signed char)13);
    h = h * 31 + (u64)(long)(x / (signed char)16); h = h * 31 + (u64)(long)(x % (signed char)16);
    h = h * 31 + (u64)(long)(x / (signed char)-16); h = h * 31 + (u64)(long)(x % (signed char)-16);
    h = h * 31 + (u64)(long)(x / (signed char)25); h = h * 31 + (u64)(long)(x % (signed char)25);
    h = h * 31 + (u64)(long)(x / (signed char)100); h = h * 31 + (u64)(long)(x % (signed char)100);
    h = h * 31 + (u64)(long)(x / (signed char)-100); h = h * 31 + (u64)(long)(x % (signed char)-100);
    h = h * 31 + (u64)(long)(x / (signed char)125); h = h * 31 + (u64)(long)(x % (signed char)125);
    h = h * 31 + (u64)(long)(x / (signed char)255); h = h * 31 + (u64)(long)(x % (signed char)255);
    h = h * 31 + (u64)(long)(x / (signed char)-128); h = h * 31 + (u64)(long)(x % (signed char)-128);
    h = h * 31 + (u64)(long)(x / (signed char)641); h = h * 31 + (u64)(long)(x % (signed char)641);
    h = h * 31 + (u64)(long)(x / (signed char)1000); h = h * 31 + (u64)(long)(x % (signed char)1000);
    h = h * 31 + (u64)(long)(x / (signed char)-1000); h = h * 31 + (u64)(long)(x % (signed char)-1000);
    h = h * 31 + (u64)(long)(x / (signed char)3600); h = h * 31 + (u64)(long)(x % (signed char)3600);
    h = h * 31 + (u64)(long)(x / (signed char)65537); h = h * 31 + (u64)(long)(x % (signed char)65537);
  }
  return h;
}
static const unsigned char u8_values[] = {(unsigned char)0, (unsigned char)1, (unsigned char)2, (unsigned char)7, (unsigned char)100, (unsigned char)127, (unsigned char)128, (unsigned char)200, (unsigned char)254, (unsigned char)255};
static u64 u8_run(void) {
  u64 h = 7;
  for (unsigned i = 0; i < sizeof u8_values / sizeof u8_values[0]; i++) {
    unsigned char x = u8_values[i];
    h = h * 31 + (u64)(long)(x / (unsigned char)2); h = h * 31 + (u64)(long)(x % (unsigned char)2);
    h = h * 31 + (u64)(long)(x / (unsigned char)3); h = h * 31 + (u64)(long)(x % (unsigned char)3);
    h = h * 31 + (u64)(long)(x / (unsigned char)5); h = h * 31 + (u64)(long)(x % (unsigned char)5);
    h = h * 31 + (u64)(long)(x / (unsigned char)6); h = h * 31 + (u64)(long)(x % (unsigned char)6);
    h = h * 31 + (u64)(long)(x / (unsigned char)7); h = h * 31 + (u64)(long)(x % (unsigned char)7);
    h = h * 31 + (u64)(long)(x / (unsigned char)9); h = h * 31 + (u64)(long)(x % (unsigned char)9);
    h = h * 31 + (u64)(long)(x / (unsigned char)10); h = h * 31 + (u64)(long)(x % (unsigned char)10);
    h = h * 31 + (u64)(long)(x / (unsigned char)11); h = h * 31 + (u64)(long)(x % (unsigned char)11);
    h = h * 31 + (u64)(long)(x / (unsigned char)12); h = h * 31 + (u64)(long)(x % (unsigned char)12);
    h = h * 31 + (u64)(long)(x / (unsigned char)13); h = h * 31 + (u64)(long)(x % (unsigned char)13);
    h = h * 31 + (u64)(long)(x / (unsigned char)15); h = h * 31 + (u64)(long)(x % (unsigned char)15);
    h = h * 31 + (u64)(long)(x / (unsigned char)16); h = h * 31 + (u64)(long)(x % (unsigned char)16);
    h = h * 31 + (u64)(long)(x / (unsigned char)25); h = h * 31 + (u64)(long)(x % (unsigned char)25);
    h = h * 31 + (u64)(long)(x / (unsigned char)60); h = h * 31 + (u64)(long)(x % (unsigned char)60);
    h = h * 31 + (u64)(long)(x / (unsigned char)100); h = h * 31 + (u64)(long)(x % (unsigned char)100);
    h = h * 31 + (u64)(long)(x / (unsigned char)125); h = h * 31 + (u64)(long)(x % (unsigned char)125);
    h = h * 31 + (u64)(long)(x / (unsigned char)255); h = h * 31 + (u64)(long)(x % (unsigned char)255);
  }
  return h;
}
static const short i16_values[] = {(short)0, (short)1, (short)(-1), (short)2, (short)(-2), (short)7, (short)(-7), (short)1000, (short)(-1000), (short)32767, (short)(-32768), (short)12345, (short)(-12345)};
static u64 i16_run(void) {
  u64 h = 7;
  for (unsigned i = 0; i < sizeof i16_values / sizeof i16_values[0]; i++) {
    short x = i16_values[i];
    h = h * 31 + (u64)(long)(x / (short)2); h = h * 31 + (u64)(long)(x % (short)2);
    h = h * 31 + (u64)(long)(x / (short)3); h = h * 31 + (u64)(long)(x % (short)3);
    h = h * 31 + (u64)(long)(x / (short)-3); h = h * 31 + (u64)(long)(x % (short)-3);
    h = h * 31 + (u64)(long)(x / (short)5); h = h * 31 + (u64)(long)(x % (short)5);
    h = h * 31 + (u64)(long)(x / (short)6); h = h * 31 + (u64)(long)(x % (short)6);
    h = h * 31 + (u64)(long)(x / (short)7); h = h * 31 + (u64)(long)(x % (short)7);
    h = h * 31 + (u64)(long)(x / (short)-7); h = h * 31 + (u64)(long)(x % (short)-7);
    h = h * 31 + (u64)(long)(x / (short)9); h = h * 31 + (u64)(long)(x % (short)9);
    h = h * 31 + (u64)(long)(x / (short)10); h = h * 31 + (u64)(long)(x % (short)10);
    h = h * 31 + (u64)(long)(x / (short)-10); h = h * 31 + (u64)(long)(x % (short)-10);
    h = h * 31 + (u64)(long)(x / (short)11); h = h * 31 + (u64)(long)(x % (short)11);
    h = h * 31 + (u64)(long)(x / (short)12); h = h * 31 + (u64)(long)(x % (short)12);
    h = h * 31 + (u64)(long)(x / (short)13); h = h * 31 + (u64)(long)(x % (short)13);
    h = h * 31 + (u64)(long)(x / (short)16); h = h * 31 + (u64)(long)(x % (short)16);
    h = h * 31 + (u64)(long)(x / (short)-16); h = h * 31 + (u64)(long)(x % (short)-16);
    h = h * 31 + (u64)(long)(x / (short)25); h = h * 31 + (u64)(long)(x % (short)25);
    h = h * 31 + (u64)(long)(x / (short)100); h = h * 31 + (u64)(long)(x % (short)100);
    h = h * 31 + (u64)(long)(x / (short)-100); h = h * 31 + (u64)(long)(x % (short)-100);
    h = h * 31 + (u64)(long)(x / (short)125); h = h * 31 + (u64)(long)(x % (short)125);
    h = h * 31 + (u64)(long)(x / (short)255); h = h * 31 + (u64)(long)(x % (short)255);
    h = h * 31 + (u64)(long)(x / (short)-256); h = h * 31 + (u64)(long)(x % (short)-256);
    h = h * 31 + (u64)(long)(x / (short)641); h = h * 31 + (u64)(long)(x % (short)641);
    h = h * 31 + (u64)(long)(x / (short)1000); h = h * 31 + (u64)(long)(x % (short)1000);
    h = h * 31 + (u64)(long)(x / (short)-1000); h = h * 31 + (u64)(long)(x % (short)-1000);
    h = h * 31 + (u64)(long)(x / (short)3600); h = h * 31 + (u64)(long)(x % (short)3600);
  }
  return h;
}
static const unsigned short u16_values[] = {(unsigned short)0, (unsigned short)1, (unsigned short)2, (unsigned short)7, (unsigned short)1000, (unsigned short)32767, (unsigned short)32768, (unsigned short)65535, (unsigned short)40000};
static u64 u16_run(void) {
  u64 h = 7;
  for (unsigned i = 0; i < sizeof u16_values / sizeof u16_values[0]; i++) {
    unsigned short x = u16_values[i];
    h = h * 31 + (u64)(long)(x / (unsigned short)2); h = h * 31 + (u64)(long)(x % (unsigned short)2);
    h = h * 31 + (u64)(long)(x / (unsigned short)3); h = h * 31 + (u64)(long)(x % (unsigned short)3);
    h = h * 31 + (u64)(long)(x / (unsigned short)5); h = h * 31 + (u64)(long)(x % (unsigned short)5);
    h = h * 31 + (u64)(long)(x / (unsigned short)6); h = h * 31 + (u64)(long)(x % (unsigned short)6);
    h = h * 31 + (u64)(long)(x / (unsigned short)7); h = h * 31 + (u64)(long)(x % (unsigned short)7);
    h = h * 31 + (u64)(long)(x / (unsigned short)9); h = h * 31 + (u64)(long)(x % (unsigned short)9);
    h = h * 31 + (u64)(long)(x / (unsigned short)10); h = h * 31 + (u64)(long)(x % (unsigned short)10);
    h = h * 31 + (u64)(long)(x / (unsigned short)11); h = h * 31 + (u64)(long)(x % (unsigned short)11);
    h = h * 31 + (u64)(long)(x / (unsigned short)12); h = h * 31 + (u64)(long)(x % (unsigned short)12);
    h = h * 31 + (u64)(long)(x / (unsigned short)13); h = h * 31 + (u64)(long)(x % (unsigned short)13);
    h = h * 31 + (u64)(long)(x / (unsigned short)15); h = h * 31 + (u64)(long)(x % (unsigned short)15);
    h = h * 31 + (u64)(long)(x / (unsigned short)16); h = h * 31 + (u64)(long)(x % (unsigned short)16);
    h = h * 31 + (u64)(long)(x / (unsigned short)25); h = h * 31 + (u64)(long)(x % (unsigned short)25);
    h = h * 31 + (u64)(long)(x / (unsigned short)60); h = h * 31 + (u64)(long)(x % (unsigned short)60);
    h = h * 31 + (u64)(long)(x / (unsigned short)100); h = h * 31 + (u64)(long)(x % (unsigned short)100);
    h = h * 31 + (u64)(long)(x / (unsigned short)125); h = h * 31 + (u64)(long)(x % (unsigned short)125);
    h = h * 31 + (u64)(long)(x / (unsigned short)255); h = h * 31 + (u64)(long)(x % (unsigned short)255);
    h = h * 31 + (u64)(long)(x / (unsigned short)256); h = h * 31 + (u64)(long)(x % (unsigned short)256);
    h = h * 31 + (u64)(long)(x / (unsigned short)641); h = h * 31 + (u64)(long)(x % (unsigned short)641);
    h = h * 31 + (u64)(long)(x / (unsigned short)1000); h = h * 31 + (u64)(long)(x % (unsigned short)1000);
    h = h * 31 + (u64)(long)(x / (unsigned short)3600); h = h * 31 + (u64)(long)(x % (unsigned short)3600);
    h = h * 31 + (u64)(long)(x / (unsigned short)65535); h = h * 31 + (u64)(long)(x % (unsigned short)65535);
  }
  return h;
}
static const int i32_values[] = {(int)0, (int)1, (int)(-1), (int)2, (int)(-2), (int)7, (int)(-7), (int)1000, (int)(-1000), (int)2147483647, (int)(-2147483648), (int)123456789, (int)(-123456789), (int)65536};
static u64 i32_run(void) {
  u64 h = 7;
  for (unsigned i = 0; i < sizeof i32_values / sizeof i32_values[0]; i++) {
    int x = i32_values[i];
    h = h * 31 + (u64)(long)(x / (int)2); h = h * 31 + (u64)(long)(x % (int)2);
    h = h * 31 + (u64)(long)(x / (int)3); h = h * 31 + (u64)(long)(x % (int)3);
    h = h * 31 + (u64)(long)(x / (int)-3); h = h * 31 + (u64)(long)(x % (int)-3);
    h = h * 31 + (u64)(long)(x / (int)5); h = h * 31 + (u64)(long)(x % (int)5);
    h = h * 31 + (u64)(long)(x / (int)6); h = h * 31 + (u64)(long)(x % (int)6);
    h = h * 31 + (u64)(long)(x / (int)7); h = h * 31 + (u64)(long)(x % (int)7);
    h = h * 31 + (u64)(long)(x / (int)-7); h = h * 31 + (u64)(long)(x % (int)-7);
    h = h * 31 + (u64)(long)(x / (int)9); h = h * 31 + (u64)(long)(x % (int)9);
    h = h * 31 + (u64)(long)(x / (int)10); h = h * 31 + (u64)(long)(x % (int)10);
    h = h * 31 + (u64)(long)(x / (int)-10); h = h * 31 + (u64)(long)(x % (int)-10);
    h = h * 31 + (u64)(long)(x / (int)11); h = h * 31 + (u64)(long)(x % (int)11);
    h = h * 31 + (u64)(long)(x / (int)12); h = h * 31 + (u64)(long)(x % (int)12);
    h = h * 31 + (u64)(long)(x / (int)13); h = h * 31 + (u64)(long)(x % (int)13);
    h = h * 31 + (u64)(long)(x / (int)16); h = h * 31 + (u64)(long)(x % (int)16);
    h = h * 31 + (u64)(long)(x / (int)-16); h = h * 31 + (u64)(long)(x % (int)-16);
    h = h * 31 + (u64)(long)(x / (int)25); h = h * 31 + (u64)(long)(x % (int)25);
    h = h * 31 + (u64)(long)(x / (int)100); h = h * 31 + (u64)(long)(x % (int)100);
    h = h * 31 + (u64)(long)(x / (int)-100); h = h * 31 + (u64)(long)(x % (int)-100);
    h = h * 31 + (u64)(long)(x / (int)125); h = h * 31 + (u64)(long)(x % (int)125);
    h = h * 31 + (u64)(long)(x / (int)255); h = h * 31 + (u64)(long)(x % (int)255);
    h = h * 31 + (u64)(long)(x / (int)-256); h = h * 31 + (u64)(long)(x % (int)-256);
    h = h * 31 + (u64)(long)(x / (int)641); h = h * 31 + (u64)(long)(x % (int)641);
    h = h * 31 + (u64)(long)(x / (int)1000); h = h * 31 + (u64)(long)(x % (int)1000);
    h = h * 31 + (u64)(long)(x / (int)-1000); h = h * 31 + (u64)(long)(x % (int)-1000);
    h = h * 31 + (u64)(long)(x / (int)3600); h = h * 31 + (u64)(long)(x % (int)3600);
    h = h * 31 + (u64)(long)(x / (int)65537); h = h * 31 + (u64)(long)(x % (int)65537);
  }
  return h;
}
static const unsigned u32_values[] = {(unsigned)0u, (unsigned)1u, (unsigned)2u, (unsigned)7u, (unsigned)1000u, (unsigned)2147483647u, (unsigned)2147483648u, (unsigned)4294967295u, (unsigned)4294967294u, (unsigned)123456789u, (unsigned)3000000000u};
static u64 u32_run(void) {
  u64 h = 7;
  for (unsigned i = 0; i < sizeof u32_values / sizeof u32_values[0]; i++) {
    unsigned x = u32_values[i];
    h = h * 31 + (u64)(long)(x / (unsigned)2); h = h * 31 + (u64)(long)(x % (unsigned)2);
    h = h * 31 + (u64)(long)(x / (unsigned)3); h = h * 31 + (u64)(long)(x % (unsigned)3);
    h = h * 31 + (u64)(long)(x / (unsigned)5); h = h * 31 + (u64)(long)(x % (unsigned)5);
    h = h * 31 + (u64)(long)(x / (unsigned)6); h = h * 31 + (u64)(long)(x % (unsigned)6);
    h = h * 31 + (u64)(long)(x / (unsigned)7); h = h * 31 + (u64)(long)(x % (unsigned)7);
    h = h * 31 + (u64)(long)(x / (unsigned)9); h = h * 31 + (u64)(long)(x % (unsigned)9);
    h = h * 31 + (u64)(long)(x / (unsigned)10); h = h * 31 + (u64)(long)(x % (unsigned)10);
    h = h * 31 + (u64)(long)(x / (unsigned)11); h = h * 31 + (u64)(long)(x % (unsigned)11);
    h = h * 31 + (u64)(long)(x / (unsigned)12); h = h * 31 + (u64)(long)(x % (unsigned)12);
    h = h * 31 + (u64)(long)(x / (unsigned)13); h = h * 31 + (u64)(long)(x % (unsigned)13);
    h = h * 31 + (u64)(long)(x / (unsigned)15); h = h * 31 + (u64)(long)(x % (unsigned)15);
    h = h * 31 + (u64)(long)(x / (unsigned)16); h = h * 31 + (u64)(long)(x % (unsigned)16);
    h = h * 31 + (u64)(long)(x / (unsigned)25); h = h * 31 + (u64)(long)(x % (unsigned)25);
    h = h * 31 + (u64)(long)(x / (unsigned)60); h = h * 31 + (u64)(long)(x % (unsigned)60);
    h = h * 31 + (u64)(long)(x / (unsigned)100); h = h * 31 + (u64)(long)(x % (unsigned)100);
    h = h * 31 + (u64)(long)(x / (unsigned)125); h = h * 31 + (u64)(long)(x % (unsigned)125);
    h = h * 31 + (u64)(long)(x / (unsigned)255); h = h * 31 + (u64)(long)(x % (unsigned)255);
    h = h * 31 + (u64)(long)(x / (unsigned)256); h = h * 31 + (u64)(long)(x % (unsigned)256);
    h = h * 31 + (u64)(long)(x / (unsigned)641); h = h * 31 + (u64)(long)(x % (unsigned)641);
    h = h * 31 + (u64)(long)(x / (unsigned)1000); h = h * 31 + (u64)(long)(x % (unsigned)1000);
    h = h * 31 + (u64)(long)(x / (unsigned)3600); h = h * 31 + (u64)(long)(x % (unsigned)3600);
    h = h * 31 + (u64)(long)(x / (unsigned)65535); h = h * 31 + (u64)(long)(x % (unsigned)65535);
    h = h * 31 + (u64)(long)(x / (unsigned)65537); h = h * 31 + (u64)(long)(x % (unsigned)65537);
    h = h * 31 + (u64)(long)(x / (unsigned)1000003); h = h * 31 + (u64)(long)(x % (unsigned)1000003);
  }
  return h;
}
static const long i64_values[] = {(long)0l, (long)1l, (long)(-1l), (long)2l, (long)(-2l), (long)7l, (long)(-7l), (long)1000l, (long)(-1000l), (long)9223372036854775807l, (long)(-9223372036854775808l), (long)1234567890123456789l, (long)(-1234567890123456789l), (long)4294967296l};
static u64 i64_run(void) {
  u64 h = 7;
  for (unsigned i = 0; i < sizeof i64_values / sizeof i64_values[0]; i++) {
    long x = i64_values[i];
    h = h * 31 + (u64)(long)(x / (long)2); h = h * 31 + (u64)(long)(x % (long)2);
    h = h * 31 + (u64)(long)(x / (long)3); h = h * 31 + (u64)(long)(x % (long)3);
    h = h * 31 + (u64)(long)(x / (long)-3); h = h * 31 + (u64)(long)(x % (long)-3);
    h = h * 31 + (u64)(long)(x / (long)5); h = h * 31 + (u64)(long)(x % (long)5);
    h = h * 31 + (u64)(long)(x / (long)6); h = h * 31 + (u64)(long)(x % (long)6);
    h = h * 31 + (u64)(long)(x / (long)7); h = h * 31 + (u64)(long)(x % (long)7);
    h = h * 31 + (u64)(long)(x / (long)-7); h = h * 31 + (u64)(long)(x % (long)-7);
    h = h * 31 + (u64)(long)(x / (long)9); h = h * 31 + (u64)(long)(x % (long)9);
    h = h * 31 + (u64)(long)(x / (long)10); h = h * 31 + (u64)(long)(x % (long)10);
    h = h * 31 + (u64)(long)(x / (long)-10); h = h * 31 + (u64)(long)(x % (long)-10);
    h = h * 31 + (u64)(long)(x / (long)11); h = h * 31 + (u64)(long)(x % (long)11);
    h = h * 31 + (u64)(long)(x / (long)12); h = h * 31 + (u64)(long)(x % (long)12);
    h = h * 31 + (u64)(long)(x / (long)13); h = h * 31 + (u64)(long)(x % (long)13);
    h = h * 31 + (u64)(long)(x / (long)16); h = h * 31 + (u64)(long)(x % (long)16);
    h = h * 31 + (u64)(long)(x / (long)-16); h = h * 31 + (u64)(long)(x % (long)-16);
    h = h * 31 + (u64)(long)(x / (long)25); h = h * 31 + (u64)(long)(x % (long)25);
    h = h * 31 + (u64)(long)(x / (long)100); h = h * 31 + (u64)(long)(x % (long)100);
    h = h * 31 + (u64)(long)(x / (long)-100); h = h * 31 + (u64)(long)(x % (long)-100);
    h = h * 31 + (u64)(long)(x / (long)125); h = h * 31 + (u64)(long)(x % (long)125);
    h = h * 31 + (u64)(long)(x / (long)255); h = h * 31 + (u64)(long)(x % (long)255);
    h = h * 31 + (u64)(long)(x / (long)-256); h = h * 31 + (u64)(long)(x % (long)-256);
    h = h * 31 + (u64)(long)(x / (long)641); h = h * 31 + (u64)(long)(x % (long)641);
    h = h * 31 + (u64)(long)(x / (long)1000); h = h * 31 + (u64)(long)(x % (long)1000);
    h = h * 31 + (u64)(long)(x / (long)-1000); h = h * 31 + (u64)(long)(x % (long)-1000);
    h = h * 31 + (u64)(long)(x / (long)3600); h = h * 31 + (u64)(long)(x % (long)3600);
    h = h * 31 + (u64)(long)(x / (long)65537); h = h * 31 + (u64)(long)(x % (long)65537);
  }
  return h;
}
static const unsigned long u64_values[] = {(unsigned long)0ul, (unsigned long)1ul, (unsigned long)2ul, (unsigned long)7ul, (unsigned long)1000ul, (unsigned long)9223372036854775807ul, (unsigned long)9223372036854775808ul, (unsigned long)18446744073709551615ul, (unsigned long)12345678901234567890ul, (unsigned long)4294967296ul};
static u64 u64_run(void) {
  u64 h = 7;
  for (unsigned i = 0; i < sizeof u64_values / sizeof u64_values[0]; i++) {
    unsigned long x = u64_values[i];
    h = h * 31 + (u64)(long)(x / (unsigned long)2); h = h * 31 + (u64)(long)(x % (unsigned long)2);
    h = h * 31 + (u64)(long)(x / (unsigned long)3); h = h * 31 + (u64)(long)(x % (unsigned long)3);
    h = h * 31 + (u64)(long)(x / (unsigned long)5); h = h * 31 + (u64)(long)(x % (unsigned long)5);
    h = h * 31 + (u64)(long)(x / (unsigned long)6); h = h * 31 + (u64)(long)(x % (unsigned long)6);
    h = h * 31 + (u64)(long)(x / (unsigned long)7); h = h * 31 + (u64)(long)(x % (unsigned long)7);
    h = h * 31 + (u64)(long)(x / (unsigned long)9); h = h * 31 + (u64)(long)(x % (unsigned long)9);
    h = h * 31 + (u64)(long)(x / (unsigned long)10); h = h * 31 + (u64)(long)(x % (unsigned long)10);
    h = h * 31 + (u64)(long)(x / (unsigned long)11); h = h * 31 + (u64)(long)(x % (unsigned long)11);
    h = h * 31 + (u64)(long)(x / (unsigned long)12); h = h * 31 + (u64)(long)(x % (unsigned long)12);
    h = h * 31 + (u64)(long)(x / (unsigned long)13); h = h * 31 + (u64)(long)(x % (unsigned long)13);
    h = h * 31 + (u64)(long)(x / (unsigned long)15); h = h * 31 + (u64)(long)(x % (unsigned long)15);
    h = h * 31 + (u64)(long)(x / (unsigned long)16); h = h * 31 + (u64)(long)(x % (unsigned long)16);
    h = h * 31 + (u64)(long)(x / (unsigned long)25); h = h * 31 + (u64)(long)(x % (unsigned long)25);
    h = h * 31 + (u64)(long)(x / (unsigned long)60); h = h * 31 + (u64)(long)(x % (unsigned long)60);
    h = h * 31 + (u64)(long)(x / (unsigned long)100); h = h * 31 + (u64)(long)(x % (unsigned long)100);
    h = h * 31 + (u64)(long)(x / (unsigned long)125); h = h * 31 + (u64)(long)(x % (unsigned long)125);
    h = h * 31 + (u64)(long)(x / (unsigned long)255); h = h * 31 + (u64)(long)(x % (unsigned long)255);
    h = h * 31 + (u64)(long)(x / (unsigned long)256); h = h * 31 + (u64)(long)(x % (unsigned long)256);
    h = h * 31 + (u64)(long)(x / (unsigned long)641); h = h * 31 + (u64)(long)(x % (unsigned long)641);
    h = h * 31 + (u64)(long)(x / (unsigned long)1000); h = h * 31 + (u64)(long)(x % (unsigned long)1000);
    h = h * 31 + (u64)(long)(x / (unsigned long)3600); h = h * 31 + (u64)(long)(x % (unsigned long)3600);
    h = h * 31 + (u64)(long)(x / (unsigned long)65535); h = h * 31 + (u64)(long)(x % (unsigned long)65535);
    h = h * 31 + (u64)(long)(x / (unsigned long)65537); h = h * 31 + (u64)(long)(x % (unsigned long)65537);
    h = h * 31 + (u64)(long)(x / (unsigned long)1000003); h = h * 31 + (u64)(long)(x % (unsigned long)1000003);
  }
  return h;
}
int main(void) {
  printf("i8 %lx\n", i8_run());
  printf("u8 %lx\n", u8_run());
  printf("i16 %lx\n", i16_run());
  printf("u16 %lx\n", u16_run());
  printf("i32 %lx\n", i32_run());
  printf("u32 %lx\n", u32_run());
  printf("i64 %lx\n", i64_run());
  printf("u64 %lx\n", u64_run());
  return 0;
}
