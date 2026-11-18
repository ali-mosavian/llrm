// flags: -O2 -march=i486 -m32 -mabi=watcom
// link: wc_small_result.wc
// targets: dos
/* A struct result of 1, 2 or 4 bytes is returned in AL, AX or EAX under Open Watcom's convention, any other through the
   address in ESI: each result crosses between wcc386's objects and llrm's. clang returns every one through a pointer
   (`sret`); the 1, 2 and 4 byte ones came back unwritten from Open Watcom's callee. */
extern void report(long value);
struct S1 { char b[1]; };
struct S2 { char b[2]; };
struct S3 { char b[3]; };
struct S4 { char b[4]; };
struct S8 { char b[8]; };

extern struct S1 wc_r1(void);
extern struct S2 wc_r2(void);
extern struct S3 wc_r3(void);
extern struct S4 wc_r4(void);
extern struct S8 wc_r8(void);
extern int wc_calls_r1(void);
extern int wc_calls_r2(void);
extern int wc_calls_r3(void);
extern int wc_calls_r4(void);
extern int wc_calls_r8(void);

struct S1 llrm_r1(void) { struct S1 s; s.b[0] = 0x51; return s; }
struct S2 llrm_r2(void) { struct S2 s; s.b[0] = 0x61; s.b[1] = 0x62; return s; }
struct S3 llrm_r3(void) { struct S3 s; s.b[0] = 0x14; s.b[1] = 0x15; s.b[2] = 0x16; return s; }
struct S4 llrm_r4(void) { struct S4 s; s.b[0] = 0x11; s.b[1] = 0x12; s.b[2] = 0x13; s.b[3] = 0x14; return s; }
struct S8 llrm_r8(void) { struct S8 s; int i; for (i = 0; i < 8; i++) s.b[i] = 0x21 + i; return s; }

int main(void)
{
    struct S1 a = wc_r1();
    struct S2 b = wc_r2();
    struct S3 c = wc_r3();
    struct S4 d = wc_r4();
    struct S8 e = wc_r8();
    report(a.b[0]);
    report(b.b[0] * 256 + b.b[1]);
    report((c.b[0] * 256 + c.b[1]) * 256 + c.b[2]);
    report(((d.b[0] * 256L + d.b[1]) * 256 + d.b[2]) * 256 + d.b[3]);
    report(((e.b[0] * 256L + e.b[1]) * 256 + e.b[2]) * 256 + e.b[7]);
    report(wc_calls_r1());
    report(wc_calls_r2());
    report(wc_calls_r3());
    report(wc_calls_r4());
    report(wc_calls_r8());
    return 0;
}
