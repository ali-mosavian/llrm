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
short f0(unsigned char p0, struct S4 p1, long long p2, float p3);
unsigned char f1(unsigned char p0);
void f2(double p0, long p1, struct S8 p2, struct S8 p3, signed char p4, unsigned short p5);
float f3(long p0, struct S8 p1, short p2, struct S12 p3, unsigned char p4);
unsigned short f4(signed char p0, struct S12 p1, short __far * p2);
struct S4 f5(struct S8 p0, long p1, unsigned short p2, short p3);
unsigned short f6(short __far * p0, struct S12 p1, struct S4 p2, struct S12 p3);
struct S8 f7(unsigned char p0, struct S12 p1, short * p2, unsigned long p3, short __far * p4);
unsigned long f8(short p0, unsigned short p1, float p2);
unsigned short f9(struct S12 p0);
struct S8 f10(signed char p0, long p1, short * p2, struct S4 p3);
struct S12 f11(struct S12 p0, struct S8 p1, double p2, unsigned short p3);
unsigned long f12(short * p0, long p1, long long p2, float p3, short p4, unsigned short p5);
unsigned short f13(struct S12 p0, struct S8 p1, signed char p2, unsigned char p3);
void f14(short __far * p0, long p1, double p2, double p3, short __far * p4, float p5, struct S4 p6);
long f15(short __far * p0, unsigned short p1);
signed char f16(unsigned short p0, short __far * p1, short __far * p2, float p3, long p4, short p5);
long f17(struct S4 p0, short * p1, unsigned long p2, short __far * p3, double p4, unsigned short p5);
struct S12 f18(unsigned char p0, struct S12 p1, signed char p2, struct S4 p3, unsigned short p4);
unsigned short f19(unsigned char p0, unsigned long p1, short p2);
unsigned short f20(signed char p0, signed char p1, unsigned long p2, struct S4 p3, unsigned char p4, long p5, struct S4 p6);
short * f21(signed char p0, unsigned long p1, long p2, unsigned long p3, short p4);
struct S4 f22(struct S12 p0, struct S12 p1, short * p2, short __far * p3, struct S12 p4, unsigned char p5);
unsigned short f23(float p0, long p1, short p2);
unsigned short f24(short * p0, short p1, unsigned long p2, short * p3, signed char p4, unsigned long p5);
void f25(short p0, unsigned long p1, struct S8 p2, struct S8 p3, unsigned long p4, long p5, short * p6);
signed char f26(unsigned short p0, long long p1, unsigned short p2, unsigned char p3, signed char p4, signed char p5, signed char p6);
short __far * f27(short * p0, short __far * p1);
unsigned char f28(void);
float f29(short * p0, unsigned short p1, unsigned long p2, signed char p3, unsigned char p4, struct S12 p5, double p6);
unsigned short f30(short __far * p0, unsigned short p1, float p2, unsigned short p3, unsigned short p4, float p5);
unsigned long f31(signed char p0, unsigned short p1, long long p2, float p3, unsigned short p4, short __far * p5, long long p6);
struct S8 f32(float p0, unsigned short p1, signed char p2);
void f33(long p0, unsigned short p1, double p2, unsigned short p3);
struct S4 f34(long long p0, struct S12 p1, long p2);
unsigned char f35(signed char p0, unsigned long p1, short * p2, unsigned char p3, short * p4);
unsigned long f36(void);
long long f37(unsigned char p0, long long p1, unsigned short p2, struct S12 p3, short * p4, short p5);
long f38(short __far * p0, float p1, struct S8 p2, short __far * p3);
long f39(double p0, unsigned short p1, short __far * p2, struct S8 p3, short __far * p4, struct S8 p5);
unsigned short f40(long long p0, float p1, unsigned char p2, struct S12 p3, long p4);
short * f41(signed char p0, long long p1, short * p2);
unsigned char f42(short __far * p0, struct S12 p1, signed char p2, struct S12 p3);
unsigned char f43(short * p0, float p1, struct S4 p2, long long p3, long long p4, unsigned short p5, struct S8 p6);
void f44(short p0, signed char p1, short * p2);
struct S12 f45(unsigned char p0, long long p1, struct S8 p2, struct S8 p3);
struct S4 f46(unsigned short p0, double p1, signed char p2, unsigned short p3, short p4, short __far * p5);
double f47(struct S12 p0, double p1, struct S8 p2, struct S8 p3, float p4);
struct S12 f48(signed char p0, unsigned char p1, signed char p2, struct S4 p3, short * p4, unsigned char p5, float p6);
float f49(short * p0, struct S8 p1, short p2, signed char p3);
long f50(struct S8 p0);
struct S12 f51(void);
unsigned short f52(struct S12 p0, unsigned char p1, unsigned char p2, double p3, float p4);
unsigned long f53(struct S8 p0, long p1, long long p2);
short f54(struct S12 p0, long long p1, unsigned char p2, unsigned char p3, unsigned char p4, short * p5, struct S8 p6);
struct S12 f55(double p0, long long p1, long long p2, struct S12 p3, struct S4 p4);
struct S4 f56(unsigned char p0, short __far * p1, unsigned short p2, short __far * p3, long p4, struct S12 p5, float p6);
unsigned long f57(struct S8 p0);
float f58(unsigned long p0, struct S12 p1);
unsigned char f59(struct S4 p0, short p1);
short f0(unsigned char p0, struct S4 p1, long long p2, float p3) {
    unsigned long h = 1;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a * 3L + p1.b);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p3);
    return (short)h;
}
unsigned char f1(unsigned char p0) {
    unsigned long h = 2;
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    return (unsigned char)h;
}
void f2(double p0, long p1, struct S8 p2, struct S8 p3, signed char p4, unsigned short p5) {
    unsigned long h = 3;
    esc(&p3);
    esc(&p2);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a + p2.b * 5L + p2.c);
    h = h * 31 + (unsigned long)(p3.a + p3.b * 5L + p3.c);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    cells[h & 7] = (short)h;
}
float f3(long p0, struct S8 p1, short p2, struct S12 p3, unsigned char p4) {
    unsigned long h = 4;
    esc(&p4);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a ^ p3.b ^ p3.c);
    h = h * 31 + (unsigned long)(long)(p4);
    return (float)(h & 0xFFF) + 0.5;
}
unsigned short f4(signed char p0, struct S12 p1, short __far * p2) {
    unsigned long h = 5;
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)*p2;
    return (unsigned short)h;
}
struct S4 f5(struct S8 p0, long p1, unsigned short p2, short p3) {
    unsigned long h = 6;
    esc(&p1);
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    { struct S4 r; r.a = (short)h; r.b = (short)(h >> 3); return r; }
}
unsigned short f6(short __far * p0, struct S12 p1, struct S4 p2, struct S12 p3) {
    unsigned long h = 7;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(p2.a * 3L + p2.b);
    h = h * 31 + (unsigned long)(p3.a ^ p3.b ^ p3.c);
    return (unsigned short)h;
}
struct S8 f7(unsigned char p0, struct S12 p1, short * p2, unsigned long p3, short __far * p4) {
    unsigned long h = 8;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)*p4;
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
unsigned long f8(short p0, unsigned short p1, float p2) {
    unsigned long h = 9;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    return (unsigned long)h;
}
unsigned short f9(struct S12 p0) {
    unsigned long h = 10;
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    return (unsigned short)h;
}
struct S8 f10(signed char p0, long p1, short * p2, struct S4 p3) {
    unsigned long h = 11;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(p3.a * 3L + p3.b);
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
struct S12 f11(struct S12 p0, struct S8 p1, double p2, unsigned short p3) {
    unsigned long h = 12;
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
unsigned long f12(short * p0, long p1, long long p2, float p3, short p4, unsigned short p5) {
    unsigned long h = 13;
    esc(&p2);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    return (unsigned long)h;
}
unsigned short f13(struct S12 p0, struct S8 p1, signed char p2, unsigned char p3) {
    unsigned long h = 14;
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    return (unsigned short)h;
}
void f14(short __far * p0, long p1, double p2, double p3, short __far * p4, float p5, struct S4 p6) {
    unsigned long h = 15;
    esc(&p2);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(p6.a * 3L + p6.b);
    cells[h & 7] = (short)h;
}
long f15(short __far * p0, unsigned short p1) {
    unsigned long h = 16;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    return (long)h;
}
signed char f16(unsigned short p0, short __far * p1, short __far * p2, float p3, long p4, short p5) {
    unsigned long h = 17;
    esc(&p5);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    return (signed char)h;
}
long f17(struct S4 p0, short * p1, unsigned long p2, short __far * p3, double p4, unsigned short p5) {
    unsigned long h = 18;
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)*p3;
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    return (long)h;
}
struct S12 f18(unsigned char p0, struct S12 p1, signed char p2, struct S4 p3, unsigned short p4) {
    unsigned long h = 19;
    esc(&p3);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a * 3L + p3.b);
    h = h * 31 + (unsigned long)(long)(p4);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
unsigned short f19(unsigned char p0, unsigned long p1, short p2) {
    unsigned long h = 20;
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    return (unsigned short)h;
}
unsigned short f20(signed char p0, signed char p1, unsigned long p2, struct S4 p3, unsigned char p4, long p5, struct S4 p6) {
    unsigned long h = 21;
    esc(&p6);
    esc(&p4);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a * 3L + p3.b);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(p6.a * 3L + p6.b);
    return (unsigned short)h;
}
short * f21(signed char p0, unsigned long p1, long p2, unsigned long p3, short p4) {
    unsigned long h = 22;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    return (short *)&cells[h & 7];
}
struct S4 f22(struct S12 p0, struct S12 p1, short * p2, short __far * p3, struct S12 p4, unsigned char p5) {
    unsigned long h = 23;
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(long)*p3;
    h = h * 31 + (unsigned long)(p4.a ^ p4.b ^ p4.c);
    h = h * 31 + (unsigned long)(long)(p5);
    { struct S4 r; r.a = (short)h; r.b = (short)(h >> 3); return r; }
}
unsigned short f23(float p0, long p1, short p2) {
    unsigned long h = 24;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    return (unsigned short)h;
}
unsigned short f24(short * p0, short p1, unsigned long p2, short * p3, signed char p4, unsigned long p5) {
    unsigned long h = 25;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)*p3;
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    return (unsigned short)h;
}
void f25(short p0, unsigned long p1, struct S8 p2, struct S8 p3, unsigned long p4, long p5, short * p6) {
    unsigned long h = 26;
    esc(&p2);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a + p2.b * 5L + p2.c);
    h = h * 31 + (unsigned long)(p3.a + p3.b * 5L + p3.c);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)*p6;
    cells[h & 7] = (short)h;
}
signed char f26(unsigned short p0, long long p1, unsigned short p2, unsigned char p3, signed char p4, signed char p5, signed char p6) {
    unsigned long h = 27;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)((p1) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)(p6);
    return (signed char)h;
}
short __far * f27(short * p0, short __far * p1) {
    unsigned long h = 28;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)*p1;
    return (short __far *)&cells[h & 7];
}
unsigned char f28(void) {
    unsigned long h = 29;
    return (unsigned char)h;
}
float f29(short * p0, unsigned short p1, unsigned long p2, signed char p3, unsigned char p4, struct S12 p5, double p6) {
    unsigned long h = 30;
    esc(&p6);
    esc(&p5);
    esc(&p3);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(p5.a ^ p5.b ^ p5.c);
    h = h * 31 + (unsigned long)(long)(p6);
    return (float)(h & 0xFFF) + 0.5;
}
unsigned short f30(short __far * p0, unsigned short p1, float p2, unsigned short p3, unsigned short p4, float p5) {
    unsigned long h = 31;
    esc(&p1);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)(p5);
    return (unsigned short)h;
}
unsigned long f31(signed char p0, unsigned short p1, long long p2, float p3, unsigned short p4, short __far * p5, long long p6) {
    unsigned long h = 32;
    esc(&p4);
    esc(&p2);
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)*p5;
    h = h * 31 + (unsigned long)(long)((p6) % 1000003LL);
    return (unsigned long)h;
}
struct S8 f32(float p0, unsigned short p1, signed char p2) {
    unsigned long h = 33;
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    { struct S8 r; r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4); return r; }
}
void f33(long p0, unsigned short p1, double p2, unsigned short p3) {
    unsigned long h = 34;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    cells[h & 7] = (short)h;
}
struct S4 f34(long long p0, struct S12 p1, long p2) {
    unsigned long h = 35;
    esc(&p2);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)((p0) % 1000003LL);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    { struct S4 r; r.a = (short)h; r.b = (short)(h >> 3); return r; }
}
unsigned char f35(signed char p0, unsigned long p1, short * p2, unsigned char p3, short * p4) {
    unsigned long h = 36;
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)*p4;
    return (unsigned char)h;
}
unsigned long f36(void) {
    unsigned long h = 37;
    return (unsigned long)h;
}
long long f37(unsigned char p0, long long p1, unsigned short p2, struct S12 p3, short * p4, short p5) {
    unsigned long h = 38;
    esc(&p5);
    esc(&p2);
    esc(&p0);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)((p1) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a ^ p3.b ^ p3.c);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    return (long long)h * 1000003LL;
}
long f38(short __far * p0, float p1, struct S8 p2, short __far * p3) {
    unsigned long h = 39;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a + p2.b * 5L + p2.c);
    h = h * 31 + (unsigned long)(long)*p3;
    return (long)h;
}
long f39(double p0, unsigned short p1, short __far * p2, struct S8 p3, short __far * p4, struct S8 p5) {
    unsigned long h = 40;
    esc(&p1);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    h = h * 31 + (unsigned long)(p3.a + p3.b * 5L + p3.c);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(p5.a + p5.b * 5L + p5.c);
    return (long)h;
}
unsigned short f40(long long p0, float p1, unsigned char p2, struct S12 p3, long p4) {
    unsigned long h = 41;
    esc(&p4);
    esc(&p3);
    h = h * 31 + (unsigned long)(long)((p0) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a ^ p3.b ^ p3.c);
    h = h * 31 + (unsigned long)(long)(p4);
    return (unsigned short)h;
}
short * f41(signed char p0, long long p1, short * p2) {
    unsigned long h = 42;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)((p1) % 1000003LL);
    h = h * 31 + (unsigned long)(long)*p2;
    return (short *)&cells[h & 7];
}
unsigned char f42(short __far * p0, struct S12 p1, signed char p2, struct S12 p3) {
    unsigned long h = 43;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a ^ p3.b ^ p3.c);
    return (unsigned char)h;
}
unsigned char f43(short * p0, float p1, struct S4 p2, long long p3, long long p4, unsigned short p5, struct S8 p6) {
    unsigned long h = 44;
    esc(&p6);
    esc(&p5);
    esc(&p4);
    esc(&p2);
    esc(&p1);
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a * 3L + p2.b);
    h = h * 31 + (unsigned long)(long)((p3) % 1000003LL);
    h = h * 31 + (unsigned long)(long)((p4) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(p6.a + p6.b * 5L + p6.c);
    return (unsigned char)h;
}
void f44(short p0, signed char p1, short * p2) {
    unsigned long h = 45;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)*p2;
    cells[h & 7] = (short)h;
}
struct S12 f45(unsigned char p0, long long p1, struct S8 p2, struct S8 p3) {
    unsigned long h = 46;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)((p1) % 1000003LL);
    h = h * 31 + (unsigned long)(p2.a + p2.b * 5L + p2.c);
    h = h * 31 + (unsigned long)(p3.a + p3.b * 5L + p3.c);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
struct S4 f46(unsigned short p0, double p1, signed char p2, unsigned short p3, short p4, short __far * p5) {
    unsigned long h = 47;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)*p5;
    { struct S4 r; r.a = (short)h; r.b = (short)(h >> 3); return r; }
}
double f47(struct S12 p0, double p1, struct S8 p2, struct S8 p3, float p4) {
    unsigned long h = 48;
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(p2.a + p2.b * 5L + p2.c);
    h = h * 31 + (unsigned long)(p3.a + p3.b * 5L + p3.c);
    h = h * 31 + (unsigned long)(long)(p4);
    return (double)(h & 0xFFF) + 0.5;
}
struct S12 f48(signed char p0, unsigned char p1, signed char p2, struct S4 p3, short * p4, unsigned char p5, float p6) {
    unsigned long h = 49;
    esc(&p5);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(p3.a * 3L + p3.b);
    h = h * 31 + (unsigned long)(long)*p4;
    h = h * 31 + (unsigned long)(long)(p5);
    h = h * 31 + (unsigned long)(long)(p6);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
float f49(short * p0, struct S8 p1, short p2, signed char p3) {
    unsigned long h = 50;
    h = h * 31 + (unsigned long)(long)*p0;
    h = h * 31 + (unsigned long)(p1.a + p1.b * 5L + p1.c);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    return (float)(h & 0xFFF) + 0.5;
}
long f50(struct S8 p0) {
    unsigned long h = 51;
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    return (long)h;
}
struct S12 f51(void) {
    unsigned long h = 52;
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
unsigned short f52(struct S12 p0, unsigned char p1, unsigned char p2, double p3, float p4) {
    unsigned long h = 53;
    esc(&p3);
    esc(&p2);
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    return (unsigned short)h;
}
unsigned long f53(struct S8 p0, long p1, long long p2) {
    unsigned long h = 54;
    esc(&p2);
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    h = h * 31 + (unsigned long)(long)(p1);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    return (unsigned long)h;
}
short f54(struct S12 p0, long long p1, unsigned char p2, unsigned char p3, unsigned char p4, short * p5, struct S8 p6) {
    unsigned long h = 55;
    esc(&p6);
    esc(&p4);
    h = h * 31 + (unsigned long)(p0.a ^ p0.b ^ p0.c);
    h = h * 31 + (unsigned long)(long)((p1) % 1000003LL);
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)(p3);
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(long)*p5;
    h = h * 31 + (unsigned long)(p6.a + p6.b * 5L + p6.c);
    return (short)h;
}
struct S12 f55(double p0, long long p1, long long p2, struct S12 p3, struct S4 p4) {
    unsigned long h = 56;
    esc(&p3);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)((p1) % 1000003LL);
    h = h * 31 + (unsigned long)(long)((p2) % 1000003LL);
    h = h * 31 + (unsigned long)(p3.a ^ p3.b ^ p3.c);
    h = h * 31 + (unsigned long)(p4.a * 3L + p4.b);
    { struct S12 r; r.a = h; r.b = h >> 1; r.c = h >> 2; return r; }
}
struct S4 f56(unsigned char p0, short __far * p1, unsigned short p2, short __far * p3, long p4, struct S12 p5, float p6) {
    unsigned long h = 57;
    esc(&p4);
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(long)*p1;
    h = h * 31 + (unsigned long)(long)(p2);
    h = h * 31 + (unsigned long)(long)*p3;
    h = h * 31 + (unsigned long)(long)(p4);
    h = h * 31 + (unsigned long)(p5.a ^ p5.b ^ p5.c);
    h = h * 31 + (unsigned long)(long)(p6);
    { struct S4 r; r.a = (short)h; r.b = (short)(h >> 3); return r; }
}
unsigned long f57(struct S8 p0) {
    unsigned long h = 58;
    h = h * 31 + (unsigned long)(p0.a + p0.b * 5L + p0.c);
    return (unsigned long)h;
}
float f58(unsigned long p0, struct S12 p1) {
    unsigned long h = 59;
    h = h * 31 + (unsigned long)(long)(p0);
    h = h * 31 + (unsigned long)(p1.a ^ p1.b ^ p1.c);
    return (float)(h & 0xFFF) + 0.5;
}
unsigned char f59(struct S4 p0, short p1) {
    unsigned long h = 60;
    esc(&p1);
    esc(&p0);
    h = h * 31 + (unsigned long)(p0.a * 3L + p0.b);
    h = h * 31 + (unsigned long)(long)(p1);
    return (unsigned char)h;
}
