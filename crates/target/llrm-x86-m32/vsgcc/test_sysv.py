"""`-mabi=sysv` against gcc -m32, which the i386 System V ABI is gcc's default on Linux: one link of llrm's object and gcc's, each
calling the other in every shape the convention has (stack arguments, i64, float and double, a struct result of any size
through a hidden pointer the callee pops, a struct by value)."""
from pathlib import Path

from test_elf import link, llrm_object, sh, work  # noqa: F401  (the fixture)

import harness

LLRM_SIDE = r"""
struct S1 { char x; };
struct S4 { int x; };
struct S8 { int x, y; };
struct S12 { int a, b, c; };
struct D { double d; };
extern struct S1 gcc_s1(int a);
extern struct S4 gcc_s4(int a);
extern struct S8 gcc_s8(int a, int b);
extern struct S12 gcc_s12(int a, int b);
extern struct D gcc_d(double a);
extern long long gcc_ll(int a, long long b, int c);
extern double gcc_dd(double a, int b, float c);
extern int gcc_by_value(struct S12 s, struct S1 t, int a);

struct S1 llrm_s1(int a) { struct S1 s; s.x = a; return s; }
struct S4 llrm_s4(int a) { struct S4 s; s.x = a + 1; return s; }
struct S8 llrm_s8(int a, int b) { struct S8 s; s.x = a; s.y = b; return s; }
struct S12 llrm_s12(int a, int b) { struct S12 s; s.a = a; s.b = b; s.c = a + b; return s; }
struct D llrm_d(double a) { struct D s; s.d = a * 2.0; return s; }
long long llrm_ll(int a, long long b, int c) { return b + b + b + a + c; }
double llrm_dd(double a, int b, float c) { return a + b + c; }
int llrm_by_value(struct S12 s, struct S1 t, int a) { return s.a + s.c + t.x + a; }
int llrm_six(int a, int b, int c, int d, int e, int f) { return a + b * 2 + c * 3 + d * 4 + e * 5 + f * 6; }

/* llrm calling gcc's: each result is folded into an int the other side prints. */
int llrm_calls_gcc(int a)
{
    struct S1 u = gcc_s1(a);
    struct S4 v = gcc_s4(a);
    struct S8 t = gcc_s8(a, 3);
    struct S12 s = gcc_s12(a, 2);
    struct S12 w;
    struct S1 one;
    struct D d = gcc_d(1.5);
    long long l = gcc_ll(1, 0x200000003LL, 4);
    w.a = 1; w.b = 2; w.c = 3;
    one.x = 5;
    return u.x + v.x * 10 + t.y * 100 + s.c * 1000 + (int)(l >> 32) * 10000 + (int)l * 100000 + gcc_by_value(w, one, 6) * 1000000 + (int)gcc_dd(2.5, 3, 4.0f) + (int)d.d;
}
"""

GCC_SIDE = r"""
struct S1 { char x; };
struct S4 { int x; };
struct S8 { int x, y; };
struct S12 { int a, b, c; };
struct D { double d; };
#define N __attribute__((noinline))
extern struct S1 llrm_s1(int a);
extern struct S4 llrm_s4(int a);
extern struct S8 llrm_s8(int a, int b);
extern struct S12 llrm_s12(int a, int b);
extern struct D llrm_d(double a);
extern long long llrm_ll(int a, long long b, int c);
extern double llrm_dd(double a, int b, float c);
extern int llrm_by_value(struct S12 s, struct S1 t, int a);
extern int llrm_six(int a, int b, int c, int d, int e, int f);
extern int llrm_calls_gcc(int a);
extern void report(int);

N struct S1 gcc_s1(int a) { struct S1 s; s.x = a; return s; }
N struct S4 gcc_s4(int a) { struct S4 s; s.x = a + 1; return s; }
N struct S8 gcc_s8(int a, int b) { struct S8 s; s.x = a; s.y = b; return s; }
N struct S12 gcc_s12(int a, int b) { struct S12 s; s.a = a; s.b = b; s.c = a + b; return s; }
N struct D gcc_d(double a) { struct D s; s.d = a * 2.0; return s; }
N long long gcc_ll(int a, long long b, int c) { return b + b + b + a + c; }
N double gcc_dd(double a, int b, float c) { return a + b + c; }
N int gcc_by_value(struct S12 s, struct S1 t, int a) { return s.a + s.c + t.x + a; }

N int bench_sysv(void)
{
    struct S12 w;
    struct S1 one;
    long long l = llrm_ll(1, 0x200000003LL, 4);
    struct S1 a = llrm_s1(7);
    struct S4 b = llrm_s4(7);
    struct S8 c = llrm_s8(7, 8);
    struct S12 d = llrm_s12(7, 8);
    struct D e = llrm_d(1.5);
    w.a = 1; w.b = 2; w.c = 3;
    one.x = 5;
    report(a.x);
    report(b.x);
    report(c.x * 100 + c.y);
    report(d.a * 10000 + d.b * 100 + d.c);
    report((int)e.d);
    report((int)(l >> 32));
    report((int)l);
    report((int)llrm_dd(2.5, 3, 4.0f));
    report(llrm_by_value(w, one, 9));
    report(llrm_six(1, 2, 3, 4, 5, 6));
    report(llrm_calls_gcc(7));
    return 0;
}

int main(void) { return bench_sysv(); }
"""


def expected():
    l = 3 * 0x200000003 + 1 + 4
    value = 7 + 8 * 10 + 8 * 100 + 15 * 1000
    value = 7 + (7 + 1) * 10 + 3 * 100 + (7 + 2) * 1000 + (l >> 32) * 10000 + (l & 0xFFFFFFFF) * 100000 + (1 + 3 + 5 + 6) * 1000000 + int(2.5 + 3 + 4.0) + int(3.0)
    wrapped = value & 0xFFFFFFFF
    return [7, 8, 7 * 100 + 8, 7 * 10000 + 8 * 100 + 15, 3, l >> 32, l & 0xFFFFFFFF, 9, 1 + 3 + 5 + 9, 91, wrapped - (1 << 32) if wrapped >= 1 << 31 else wrapped]


def test_llrm_sysv_and_gcc_call_each_other_in_every_shape(work):  # noqa: F811
    llrm = work / "llrm_side.c"
    llrm.write_text(LLRM_SIDE)
    gcc = work / "gcc_side.c"
    gcc.write_text(GCC_SIDE)
    first = llrm_object(work, llrm, work / "llrm_side.o", "-O2", "-mabi=sysv")
    second = work / "gcc_side.o"
    sh("gcc", "-m32", "-march=i486", "-fno-pic", "-fno-stack-protector", "-fcf-protection=none", "-fno-asynchronous-unwind-tables", "-O2", "-c", "-o", second, gcc)
    link(work, "sysv.gccO2", first, second)
    assert harness.run("sysv", "gccO2")["reports"] == expected()
