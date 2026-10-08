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
struct S12 f0(unsigned short p0, signed char p1);
double f1(short p0, struct S4 p1, unsigned char p2, struct S4 p3, short __far * p4, unsigned long p5, struct S8 p6);
struct S12 f2(unsigned short p0);
unsigned short f3(void);
long f4(double p0, short __far * p1);
short __far * f5(struct S4 p0, signed char p1, short * p2, signed char p3, short * p4, long p5);
unsigned short f6(unsigned short p0, short p1, struct S12 p2, unsigned char p3, short * p4, double p5);
short __far * f7(void);
short __far * f8(short * p0, unsigned long p1, long p2, short p3, long long p4);
unsigned short f9(long p0, struct S12 p1);
unsigned short f10(long long p0, signed char p1);
long f11(void);
unsigned long f12(double p0, long p1, double p2, long long p3);
unsigned long f13(long p0, short * p1);
long f14(short * p0, signed char p1, long p2, short * p3, short * p4, double p5);
float f15(struct S8 p0, short __far * p1, double p2, long long p3, unsigned char p4, unsigned char p5);
long long f16(unsigned char p0, long long p1, struct S8 p2, long long p3, signed char p4);
struct S12 f17(double p0, unsigned short p1, long long p2);
void f18(float p0);
struct S12 f19(short p0, signed char p1, double p2);
unsigned long f20(short p0, unsigned long p1, unsigned long p2, short __far * p3, unsigned short p4);
long long f21(void);
signed char f22(struct S8 p0, struct S8 p1);
short * f23(short __far * p0, struct S4 p1, unsigned char p2);
void f24(long long p0, unsigned char p1, struct S12 p2);
unsigned char f25(signed char p0, struct S12 p1, unsigned char p2, float p3, struct S4 p4, double p5, short p6);
double f26(short __far * p0, struct S4 p1, struct S12 p2, float p3);
long long f27(short __far * p0);
short __far * f28(struct S8 p0, unsigned char p1, struct S12 p2, struct S8 p3, short * p4);
unsigned short f29(signed char p0);
unsigned char f30(short __far * p0, unsigned char p1, unsigned long p2, unsigned char p3, short p4);
short * f31(struct S4 p0, long p1, short p2, signed char p3, unsigned char p4, unsigned short p5);
long f32(unsigned short p0);
float f33(float p0, struct S12 p1, unsigned char p2, short p3);
void f34(struct S8 p0, unsigned char p1, short * p2, float p3);
double f35(long long p0, unsigned long p1, unsigned char p2);
short __far * f36(signed char p0, unsigned short p1);
unsigned long f37(void);
short f38(struct S8 p0, float p1, short __far * p2);
unsigned long f39(void);
struct S4 f40(float p0, long p1, unsigned short p2, unsigned short p3, unsigned char p4, long p5);
struct S12 f41(short __far * p0);
double f42(short p0, short __far * p1);
long f43(void);
void f44(short p0, short * p1, long p2, signed char p3, struct S12 p4, float p5, struct S4 p6);
void f45(struct S8 p0, struct S8 p1, signed char p2, unsigned short p3);
short * f46(struct S12 p0, short __far * p1, unsigned short p2, float p3, double p4, long long p5, short p6);
long f47(long p0, struct S12 p1, struct S12 p2, struct S4 p3, signed char p4);
short __far * f48(short * p0, short __far * p1, unsigned char p2, short p3, struct S12 p4, struct S8 p5);
struct S12 f49(long p0, struct S12 p1, struct S12 p2, double p3, long p4);
short f50(long p0, double p1, short p2, unsigned long p3);
struct S8 f51(struct S4 p0, float p1, struct S12 p2, short p3, double p4, unsigned short p5, float p6);
unsigned long f52(struct S12 p0, struct S8 p1, unsigned char p2, long long p3, short __far * p4);
short * f53(struct S4 p0, short p1, unsigned char p2);
void f54(unsigned long p0, unsigned short p1, signed char p2, double p3, double p4, struct S12 p5);
unsigned char f55(double p0, unsigned long p1, struct S12 p2);
double f56(void);
float f57(struct S8 p0, struct S8 p1, signed char p2, unsigned char p3, double p4);
short * f58(void);
struct S4 f59(unsigned char p0);
struct S12 f0(unsigned short p0, signed char p1) {
    unsigned long h = 1;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
double f1(short p0, struct S4 p1, unsigned char p2, struct S4 p3, short __far * p4, unsigned long p5, struct S8 p6) {
    unsigned long h = 2;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a * 3L + p1.b);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a * 3L + p3.b);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(p6.a + p6.b * 5L + p6.c);
    return (double)(h & 0xFFF) + 0.5;
}
struct S12 f2(unsigned short p0) {
    unsigned long h = 3;
    h = h * 31 + (unsigned long)(long)(p0);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
unsigned short f3(void) {
    unsigned long h = 4;
    return (unsigned short)h;
}
long f4(double p0, short __far * p1) {
    unsigned long h = 5;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    return (long)h;
}
short __far * f5(struct S4 p0, signed char p1, short * p2, signed char p3, short * p4, long p5) {
    unsigned long h = 6;
    esc(&p5);
    esc(&p3);
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    return (short __far *)&cells[h & 7];
}
unsigned short f6(unsigned short p0, short p1, struct S12 p2, unsigned char p3, short * p4, double p5) {
    unsigned long h = 7;
    esc(&p5);
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    return (unsigned short)h;
}
short __far * f7(void) {
    unsigned long h = 8;
    return (short __far *)&cells[h & 7];
}
short __far * f8(short * p0, unsigned long p1, long p2, short p3, long long p4) {
    unsigned long h = 9;
    esc(&p1);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)((p4) % 1000003LL);
    return (short __far *)&cells[h & 7];
}
unsigned short f9(long p0, struct S12 p1) {
    unsigned long h = 10;
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    return (unsigned short)h;
}
unsigned short f10(long long p0, signed char p1) {
    unsigned long h = 11;
    h = h * 31 + (unsigned long)(long)((p0) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p1);
    return (unsigned short)h;
}
long f11(void) {
    unsigned long h = 12;
    return (long)h;
}
unsigned long f12(double p0, long p1, double p2, long long p3) {
    unsigned long h = 13;
    esc(&p3);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)((p3) % 1000003LL);
    return (unsigned long)h;
}
unsigned long f13(long p0, short * p1) {
    unsigned long h = 14;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    return (unsigned long)h;
}
long f14(short * p0, signed char p1, long p2, short * p3, short * p4, double p5) {
    unsigned long h = 15;
    esc(&p2);
    esc(&p1);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)*p3;
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    return (long)h;
}
float f15(struct S8 p0, short __far * p1, double p2, long long p3, unsigned char p4, unsigned char p5) {
    unsigned long h = 16;
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)((p3) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    return (float)(h & 0xFFF) + 0.5;
}
long long f16(unsigned char p0, long long p1, struct S8 p2, long long p3, signed char p4) {
    unsigned long h = 17;
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)((p1) % 1000003LL);
    h = h * 31 + (unsigned long)(p2.a + p2.b * 5L + p2.c);
    h = h * 31 + (unsigned long)(long)((p3) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p4);
    return (long long)h * 1000003LL;
}
struct S12 f17(double p0, unsigned short p1, long long p2) {
    unsigned long h = 18;
    esc(&p2);
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
void f18(float p0) {
    unsigned long h = 19;
    h = h * 31 + (unsigned long)(long)(p0);
    cells[h & 7] = (short)h;
}
struct S12 f19(short p0, signed char p1, double p2) {
    unsigned long h = 20;
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
unsigned long f20(short p0, unsigned long p1, unsigned long p2, short __far * p3, unsigned short p4) {
    unsigned long h = 21;
    esc(&p2);
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)*p3;
    h = h * 31 + (unsigned long)(long)(p4);
    return (unsigned long)h;
}
long long f21(void) {
    unsigned long h = 22;
    return (long long)h * 1000003LL;
}
signed char f22(struct S8 p0, struct S8 p1) {
    unsigned long h = 23;
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    return (signed char)h;
}
short * f23(short __far * p0, struct S4 p1, unsigned char p2) {
    unsigned long h = 24;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(p1.a * 3L + p1.b);
    h = h * 31 + (unsigned long)(long)(p2);
    return (short *)&cells[h & 7];
}
void f24(long long p0, unsigned char p1, struct S12 p2) {
    unsigned long h = 25;
    h = h * 31 + (unsigned long)(long)((p0) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    cells[h & 7] = (short)h;
}
unsigned char f25(signed char p0, struct S12 p1, unsigned char p2, float p3, struct S4 p4, double p5, short p6) {
    unsigned long h = 26;
    esc(&p6);
    esc(&p3);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(p4.a * 3L + p4.b);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)(p6);
    return (unsigned char)h;
}
double f26(short __far * p0, struct S4 p1, struct S12 p2, float p3) {
    unsigned long h = 27;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(p1.a * 3L + p1.b);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    h = h * 31 + (unsigned long)(long)(p3);
    return (double)(h & 0xFFF) + 0.5;
}
long long f27(short __far * p0) {
    unsigned long h = 28;
    h = h * 31 + (unsigned long)(long)*p0;
    return (long long)h * 1000003LL;
}
short __far * f28(struct S8 p0, unsigned char p1, struct S12 p2, struct S8 p3, short * p4) {
    unsigned long h = 29;
    esc(&p2);
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    h = h * 31 + (unsigned long)(p3.a + p3.b * 5L + p3.c);
    h = h * 31 + (unsigned long)(long)*p4;
    return (short __far *)&cells[h & 7];
}
unsigned short f29(signed char p0) {
    unsigned long h = 30;
    h = h * 31 + (unsigned long)(long)(p0);
    return (unsigned short)h;
}
unsigned char f30(short __far * p0, unsigned char p1, unsigned long p2, unsigned char p3, short p4) {
    unsigned long h = 31;
    esc(&p2);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    return (unsigned char)h;
}
short * f31(struct S4 p0, long p1, short p2, signed char p3, unsigned char p4, unsigned short p5) {
    unsigned long h = 32;
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    return (short *)&cells[h & 7];
}
long f32(unsigned short p0) {
    unsigned long h = 33;
    h = h * 31 + (unsigned long)(long)(p0);
    return (long)h;
}
float f33(float p0, struct S12 p1, unsigned char p2, short p3) {
    unsigned long h = 34;
    esc(&p2);
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    return (float)(h & 0xFFF) + 0.5;
}
void f34(struct S8 p0, unsigned char p1, short * p2, float p3) {
    unsigned long h = 35;
    esc(&p3);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(long)(p3);
    cells[h & 7] = (short)h;
}
double f35(long long p0, unsigned long p1, unsigned char p2) {
    unsigned long h = 36;
    esc(&p2);
    h = h * 31 + (unsigned long)(long)((p0) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    return (double)(h & 0xFFF) + 0.5;
}
short __far * f36(signed char p0, unsigned short p1) {
    unsigned long h = 37;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    return (short __far *)&cells[h & 7];
}
unsigned long f37(void) {
    unsigned long h = 38;
    return (unsigned long)h;
}
short f38(struct S8 p0, float p1, short __far * p2) {
    unsigned long h = 39;
    esc(&p1);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    return (short)h;
}
unsigned long f39(void) {
    unsigned long h = 40;
    return (unsigned long)h;
}
struct S4 f40(float p0, long p1, unsigned short p2, unsigned short p3, unsigned char p4, long p5) {
    unsigned long h = 41;
    esc(&p4);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    { struct S4 r; r.a = (short)h; r.b = (short)(h >> 3); return r; }
}
struct S12 f41(short __far * p0) {
    unsigned long h = 42;
    h = h * 31 + (unsigned long)(long)*p0;
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
double f42(short p0, short __far * p1) {
    unsigned long h = 43;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    return (double)(h & 0xFFF) + 0.5;
}
long f43(void) {
    unsigned long h = 44;
    return (long)h;
}
void f44(short p0, short * p1, long p2, signed char p3, struct S12 p4, float p5, struct S4 p6) {
    unsigned long h = 45;
    esc(&p2);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(p4.a ^ p4.b ^ p4.c);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(p6.a * 3L + p6.b);
    cells[h & 7] = (short)h;
}
void f45(struct S8 p0, struct S8 p1, signed char p2, unsigned short p3) {
    unsigned long h = 46;
    esc(&p3);
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    cells[h & 7] = (short)h;
}
short * f46(struct S12 p0, short __far * p1, unsigned short p2, float p3, double p4, long long p5, short p6) {
    unsigned long h = 47;
    esc(&p5);
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)((p5) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p6);
    return (short *)&cells[h & 7];
}
long f47(long p0, struct S12 p1, struct S12 p2, struct S4 p3, signed char p4) {
    unsigned long h = 48;
    esc(&p3);
    esc(&p2);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    h = h * 31 + (unsigned long)(p3.a * 3L + p3.b);
    h = h * 31 + (unsigned long)(long)(p4);
    return (long)h;
}
short __far * f48(short * p0, short __far * p1, unsigned char p2, short p3, struct S12 p4, struct S8 p5) {
    unsigned long h = 49;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(p4.a ^ p4.b ^ p4.c);
    h = h * 31 + (unsigned long)(p5.a + p5.b * 5L + p5.c);
    return (short __far *)&cells[h & 7];
}
struct S12 f49(long p0, struct S12 p1, struct S12 p2, double p3, long p4) {
    unsigned long h = 50;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
short f50(long p0, double p1, short p2, unsigned long p3) {
    unsigned long h = 51;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    return (short)h;
}
struct S8 f51(struct S4 p0, float p1, struct S12 p2, short p3, double p4, unsigned short p5, float p6) {
    unsigned long h = 52;
    esc(&p6);
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)(p6);
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
unsigned long f52(struct S12 p0, struct S8 p1, unsigned char p2, long long p3, short __far * p4) {
    unsigned long h = 53;
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)((p3) % 1000003LL);
    h = h * 31 + (unsigned long)(long)*p4;
    return (unsigned long)h;
}
short * f53(struct S4 p0, short p1, unsigned char p2) {
    unsigned long h = 54;
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    return (short *)&cells[h & 7];
}
void f54(unsigned long p0, unsigned short p1, signed char p2, double p3, double p4, struct S12 p5) {
    unsigned long h = 55;
    esc(&p5);
    esc(&p3);
    esc(&p2);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(p5.a ^ p5.b ^ p5.c);
    cells[h & 7] = (short)h;
}
unsigned char f55(double p0, unsigned long p1, struct S12 p2) {
    unsigned long h = 56;
    esc(&p2);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a ^ p2.b ^ p2.c);
    return (unsigned char)h;
}
double f56(void) {
    unsigned long h = 57;
    return (double)(h & 0xFFF) + 0.5;
}
float f57(struct S8 p0, struct S8 p1, signed char p2, unsigned char p3, double p4) {
    unsigned long h = 58;
    esc(&p3);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    return (float)(h & 0xFFF) + 0.5;
}
short * f58(void) {
    unsigned long h = 59;
    return (short *)&cells[h & 7];
}
struct S4 f59(unsigned char p0) {
    unsigned long h = 60;
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    { struct S4 r; r.a = (short)h; r.b = (short)(h >> 3); return r; }
}
