typedef signed char i8_t;
struct S4 { short a, b; };
struct S8 { long a; short b, c; };
struct S12 { long a, b, c; };
extern void report(long value);
extern short cells[8];

signed char id_i8(signed char x) { return x; }
unsigned char id_u8(unsigned char x) { return x; }
short id_i16(short x) { return x; }
unsigned short id_u16(unsigned short x) { return x; }
long id_i32(long x) { return x; }
unsigned long id_u32(unsigned long x) { return x; }
long long id_i64(long long x) { return x; }
float id_f32(float x) { return x; }
double id_f64(double x) { return x; }
short * id_np(short * x) { return x; }
short __far * id_fp(short __far * x) { return x; }
struct S4 id_s4(struct S4 x) { return x; }
struct S8 id_s8(struct S8 x) { return x; }
struct S12 id_s12(struct S12 x) { return x; }
short cells[8] = {11, -22, 33, -44, 55, -66, 77, -88};
void esc(void *p) { cells[7] += (short)*(unsigned char *)p; }
void f0(unsigned char p0, struct S8 p1, long long p2, short p3, short __far * p4, signed char p5, short p6);
signed char f1(struct S4 p0, struct S8 p1, short p2);
unsigned char f2(void);
short * f3(void);
unsigned char f4(double p0, struct S4 p1, float p2);
float f5(unsigned short p0, short * p1, unsigned char p2, unsigned char p3, signed char p4, struct S8 p5);
struct S12 f6(unsigned long p0, unsigned short p1, unsigned short p2, unsigned long p3, long long p4, unsigned long p5);
double f7(unsigned short p0, struct S4 p1, unsigned short p2);
unsigned long f8(void);
void f9(unsigned long p0, short p1);
short f10(struct S12 p0, struct S8 p1);
unsigned long f11(void);
signed char f12(short __far * p0, struct S8 p1, unsigned short p2, unsigned short p3, struct S8 p4, long long p5, struct S4 p6);
long f13(unsigned long p0, short __far * p1, struct S12 p2);
short f14(signed char p0, long p1, unsigned long p2, long p3, unsigned long p4, short * p5, long long p6);
long long f15(unsigned short p0, struct S4 p1, unsigned char p2, struct S8 p3, struct S8 p4);
unsigned short f16(double p0, short * p1);
struct S12 f17(unsigned char p0, short p1, unsigned short p2, signed char p3);
struct S12 f18(struct S4 p0, unsigned char p1, short __far * p2, long long p3, long p4);
struct S12 f19(short __far * p0, struct S12 p1, unsigned short p2, unsigned char p3);
long long f20(struct S8 p0, unsigned long p1, float p2);
unsigned long f21(void);
float f22(long p0);
signed char f23(unsigned short p0);
short f24(short p0, struct S4 p1, struct S12 p2, short p3, float p4);
void f25(short * p0, struct S8 p1, double p2, long p3, struct S12 p4, unsigned long p5, short * p6);
long long f26(void);
struct S8 f27(float p0);
unsigned char f28(struct S12 p0, struct S8 p1, signed char p2, long p3, float p4, struct S8 p5, struct S8 p6);
long long f29(unsigned short p0, signed char p1, unsigned char p2, long long p3, short * p4);
unsigned short f30(short p0, signed char p1, short p2, unsigned char p3, short * p4, struct S12 p5, short * p6);
void f31(struct S8 p0, unsigned short p1, struct S12 p2);
struct S8 f32(struct S4 p0, short p1, double p2, float p3);
float f33(unsigned short p0, double p1, unsigned short p2, struct S4 p3, short * p4, float p5);
unsigned short f34(unsigned long p0, short p1, unsigned long p2, unsigned char p3, double p4, struct S8 p5, unsigned long p6);
short __far * f35(long p0, struct S4 p1, long long p2);
double f36(signed char p0, struct S12 p1);
short * f37(struct S4 p0);
unsigned long f38(short p0, unsigned short p1, long long p2);
struct S12 f39(unsigned short p0, long long p1, struct S4 p2, long p3);
float f40(struct S8 p0, long p1, struct S4 p2);
struct S8 f41(void);
void f42(short * p0, struct S12 p1, long p2, long p3, double p4, unsigned short p5, long long p6);
struct S8 f43(unsigned short p0, long long p1, signed char p2, struct S12 p3, float p4);
unsigned long f44(unsigned short p0, unsigned short p1, unsigned long p2);
long f45(struct S4 p0, unsigned long p1, unsigned char p2, struct S8 p3);
short f46(unsigned short p0);
struct S12 f47(float p0, float p1, short __far * p2, long p3, short __far * p4, short p5, long long p6);
void f48(float p0);
unsigned char f49(struct S8 p0, float p1, struct S12 p2, signed char p3);
void f50(long long p0, long p1, struct S8 p2, long p3);
unsigned short f51(short * p0, long p1, long long p2);
struct S12 f52(unsigned short p0, short * p1, short p2);
struct S8 f53(struct S12 p0, unsigned char p1, short __far * p2, short * p3, struct S4 p4);
short f54(void);
long f55(long p0, short __far * p1, float p2, double p3, struct S12 p4, long p5, long p6);
struct S4 f56(long long p0, short __far * p1, unsigned short p2, double p3, unsigned short p4, struct S8 p5);
unsigned long f57(void);
struct S8 f58(struct S4 p0, unsigned short p1, signed char p2);
struct S8 f59(short __far * p0, unsigned long p1, long p2, unsigned char p3, struct S8 p4, double p5);
void f0(unsigned char p0, struct S8 p1, long long p2, short p3, short __far * p4, signed char p5, short p6) {
    unsigned long h = 1;
    esc(&p6);
    esc(&p5);
    esc(&p1);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)(p6);
    cells[h & 7] = (short)h;
}
signed char f1(struct S4 p0, struct S8 p1, short p2) {
    unsigned long h = 2;
    esc(&p1);
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    return (signed char)h;
}
unsigned char f2(void) {
    unsigned long h = 3;
    return (unsigned char)h;
}
short * f3(void) {
    unsigned long h = 4;
    return (short *)&cells[h & 7];
}
unsigned char f4(double p0, struct S4 p1, float p2) {
    unsigned long h = 5;
    esc(&p2);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a * 3L + p1.b);
    h = h * 31 + (unsigned long)(long)(p2);
    return (unsigned char)h;
}
float f5(unsigned short p0, short * p1, unsigned char p2, unsigned char p3, signed char p4, struct S8 p5) {
    unsigned long h = 6;
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(p5.a + p5.b * 5L + p5.c);
    return (float)(h & 0xFFF) + 0.5;
}
struct S12 f6(unsigned long p0, unsigned short p1, unsigned short p2, unsigned long p3, long long p4, unsigned long p5) {
    unsigned long h = 7;
    esc(&p5);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)((p4) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p5);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
double f7(unsigned short p0, struct S4 p1, unsigned short p2) {
    unsigned long h = 8;
    esc(&p2);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a * 3L + p1.b);
    h = h * 31 + (unsigned long)(long)(p2);
    return (double)(h & 0xFFF) + 0.5;
}
unsigned long f8(void) {
    unsigned long h = 9;
    return (unsigned long)h;
}
void f9(unsigned long p0, short p1) {
    unsigned long h = 10;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    cells[h & 7] = (short)h;
}
short f10(struct S12 p0, struct S8 p1) {
    unsigned long h = 11;
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    return (short)h;
}
unsigned long f11(void) {
    unsigned long h = 12;
    return (unsigned long)h;
}
signed char f12(short __far * p0, struct S8 p1, unsigned short p2, unsigned short p3, struct S8 p4, long long p5, struct S4 p6) {
    unsigned long h = 13;
    esc(&p5);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(p4.a + p4.b * 5L + p4.c);
    h = h * 31 + (unsigned long)(long)((p5) % 1000003LL);
    h = h * 31 + (unsigned long)(p6.a * 3L + p6.b);
    return (signed char)h;
}
long f13(unsigned long p0, short __far * p1, struct S12 p2) {
    unsigned long h = 14;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    return (long)h;
}
short f14(signed char p0, long p1, unsigned long p2, long p3, unsigned long p4, short * p5, long long p6) {
    unsigned long h = 15;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)*p5;
    h = h * 31 + (unsigned long)(long)((p6) % 1000003LL);
    return (short)h;
}
long long f15(unsigned short p0, struct S4 p1, unsigned char p2, struct S8 p3, struct S8 p4) {
    unsigned long h = 16;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a * 3L + p1.b);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a + p3.b * 5L + p3.c);
    h = h * 31 + (unsigned long)(p4.a + p4.b * 5L + p4.c);
    return (long long)h * 1000003LL;
}
unsigned short f16(double p0, short * p1) {
    unsigned long h = 17;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    return (unsigned short)h;
}
struct S12 f17(unsigned char p0, short p1, unsigned short p2, signed char p3) {
    unsigned long h = 18;
    esc(&p3);
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
struct S12 f18(struct S4 p0, unsigned char p1, short __far * p2, long long p3, long p4) {
    unsigned long h = 19;
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(long)((p3) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p4);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
struct S12 f19(short __far * p0, struct S12 p1, unsigned short p2, unsigned char p3) {
    unsigned long h = 20;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
long long f20(struct S8 p0, unsigned long p1, float p2) {
    unsigned long h = 21;
    esc(&p2);
    esc(&p1);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    return (long long)h * 1000003LL;
}
unsigned long f21(void) {
    unsigned long h = 22;
    return (unsigned long)h;
}
float f22(long p0) {
    unsigned long h = 23;
    h = h * 31 + (unsigned long)(long)(p0);
    return (float)(h & 0xFFF) + 0.5;
}
signed char f23(unsigned short p0) {
    unsigned long h = 24;
    h = h * 31 + (unsigned long)(long)(p0);
    return (signed char)h;
}
short f24(short p0, struct S4 p1, struct S12 p2, short p3, float p4) {
    unsigned long h = 25;
    esc(&p2);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a * 3L + p1.b);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    return (short)h;
}
void f25(short * p0, struct S8 p1, double p2, long p3, struct S12 p4, unsigned long p5, short * p6) {
    unsigned long h = 26;
    esc(&p4);
    esc(&p3);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(p4.a ^ p4.b ^ p4.c);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)*p6;
    cells[h & 7] = (short)h;
}
long long f26(void) {
    unsigned long h = 27;
    return (long long)h * 1000003LL;
}
struct S8 f27(float p0) {
    unsigned long h = 28;
    h = h * 31 + (unsigned long)(long)(p0);
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
unsigned char f28(struct S12 p0, struct S8 p1, signed char p2, long p3, float p4, struct S8 p5, struct S8 p6) {
    unsigned long h = 29;
    esc(&p4);
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(p5.a + p5.b * 5L + p5.c);
    h = h * 31 + (unsigned long)(p6.a + p6.b * 5L + p6.c);
    return (unsigned char)h;
}
long long f29(unsigned short p0, signed char p1, unsigned char p2, long long p3, short * p4) {
    unsigned long h = 30;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)((p3) % 1000003LL);
    h = h * 31 + (unsigned long)(long)*p4;
    return (long long)h * 1000003LL;
}
unsigned short f30(short p0, signed char p1, short p2, unsigned char p3, short * p4, struct S12 p5, short * p6) {
    unsigned long h = 31;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(p5.a ^ p5.b ^ p5.c);
    h = h * 31 + (unsigned long)(long)*p6;
    return (unsigned short)h;
}
void f31(struct S8 p0, unsigned short p1, struct S12 p2) {
    unsigned long h = 32;
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    cells[h & 7] = (short)h;
}
struct S8 f32(struct S4 p0, short p1, double p2, float p3) {
    unsigned long h = 33;
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
float f33(unsigned short p0, double p1, unsigned short p2, struct S4 p3, short * p4, float p5) {
    unsigned long h = 34;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a * 3L + p3.b);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    return (float)(h & 0xFFF) + 0.5;
}
unsigned short f34(unsigned long p0, short p1, unsigned long p2, unsigned char p3, double p4, struct S8 p5, unsigned long p6) {
    unsigned long h = 35;
    esc(&p6);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(p5.a + p5.b * 5L + p5.c);
    h = h * 31 + (unsigned long)(long)(p6);
    return (unsigned short)h;
}
short __far * f35(long p0, struct S4 p1, long long p2) {
    unsigned long h = 36;
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a * 3L + p1.b);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    return (short __far *)&cells[h & 7];
}
double f36(signed char p0, struct S12 p1) {
    unsigned long h = 37;
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    return (double)(h & 0xFFF) + 0.5;
}
short * f37(struct S4 p0) {
    unsigned long h = 38;
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    return (short *)&cells[h & 7];
}
unsigned long f38(short p0, unsigned short p1, long long p2) {
    unsigned long h = 39;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    return (unsigned long)h;
}
struct S12 f39(unsigned short p0, long long p1, struct S4 p2, long p3) {
    unsigned long h = 40;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)((p1) % 1000003LL);
    h = h * 31 + (unsigned long)(p2.a * 3L + p2.b);
    h = h * 31 + (unsigned long)(long)(p3);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
float f40(struct S8 p0, long p1, struct S4 p2) {
    unsigned long h = 41;
    esc(&p2);
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a * 3L + p2.b);
    return (float)(h & 0xFFF) + 0.5;
}
struct S8 f41(void) {
    unsigned long h = 42;
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
void f42(short * p0, struct S12 p1, long p2, long p3, double p4, unsigned short p5, long long p6) {
    unsigned long h = 43;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)((p6) % 1000003LL);
    cells[h & 7] = (short)h;
}
struct S8 f43(unsigned short p0, long long p1, signed char p2, struct S12 p3, float p4) {
    unsigned long h = 44;
    esc(&p4);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)((p1) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a ^ p3.b ^ p3.c);
    h = h * 31 + (unsigned long)(long)(p4);
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
unsigned long f44(unsigned short p0, unsigned short p1, unsigned long p2) {
    unsigned long h = 45;
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    return (unsigned long)h;
}
long f45(struct S4 p0, unsigned long p1, unsigned char p2, struct S8 p3) {
    unsigned long h = 46;
    esc(&p2);
    esc(&p1);
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a + p3.b * 5L + p3.c);
    return (long)h;
}
short f46(unsigned short p0) {
    unsigned long h = 47;
    h = h * 31 + (unsigned long)(long)(p0);
    return (short)h;
}
struct S12 f47(float p0, float p1, short __far * p2, long p3, short __far * p4, short p5, long long p6) {
    unsigned long h = 48;
    esc(&p5);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)((p6) % 1000003LL);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
void f48(float p0) {
    unsigned long h = 49;
    h = h * 31 + (unsigned long)(long)(p0);
    cells[h & 7] = (short)h;
}
unsigned char f49(struct S8 p0, float p1, struct S12 p2, signed char p3) {
    unsigned long h = 50;
    esc(&p3);
    esc(&p2);
    esc(&p1);
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    h = h * 31 + (unsigned long)(long)(p3);
    return (unsigned char)h;
}
void f50(long long p0, long p1, struct S8 p2, long p3) {
    unsigned long h = 51;
    h = h * 31 + (unsigned long)(long)((p0) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a + p2.b * 5L + p2.c);
    h = h * 31 + (unsigned long)(long)(p3);
    cells[h & 7] = (short)h;
}
unsigned short f51(short * p0, long p1, long long p2) {
    unsigned long h = 52;
    esc(&p2);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    return (unsigned short)h;
}
struct S12 f52(unsigned short p0, short * p1, short p2) {
    unsigned long h = 53;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
struct S8 f53(struct S12 p0, unsigned char p1, short __far * p2, short * p3, struct S4 p4) {
    unsigned long h = 54;
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(long)*p3;
    h = h * 31 + (unsigned long)(p4.a * 3L + p4.b);
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
short f54(void) {
    unsigned long h = 55;
    return (short)h;
}
long f55(long p0, short __far * p1, float p2, double p3, struct S12 p4, long p5, long p6) {
    unsigned long h = 56;
    esc(&p5);
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(p4.a ^ p4.b ^ p4.c);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)(p6);
    return (long)h;
}
struct S4 f56(long long p0, short __far * p1, unsigned short p2, double p3, unsigned short p4, struct S8 p5) {
    unsigned long h = 57;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)((p0) % 1000003LL);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(p5.a + p5.b * 5L + p5.c);
    { struct S4 r; r.a = (short)h; r.b = (short)(h >> 3); return r; }
}
unsigned long f57(void) {
    unsigned long h = 58;
    return (unsigned long)h;
}
struct S8 f58(struct S4 p0, unsigned short p1, signed char p2) {
    unsigned long h = 59;
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
struct S8 f59(short __far * p0, unsigned long p1, long p2, unsigned char p3, struct S8 p4, double p5) {
    unsigned long h = 60;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(p4.a + p4.b * 5L + p4.c);
    h = h * 31 + (unsigned long)(long)(p5);
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
