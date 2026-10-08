// flags: -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* An i64 parameter whose address is taken has a home that is written on entry, whatever register or stack cell the value arrived in:
   its high dword read through an `int` pointer was read from a cell nothing wrote (the store went as dead on the types' say-so, and the
   selector had no i64 store at all: a union of it was refused, "a i64 value"). */
extern void report(long value);
union U { long long q; int w[2]; };

int through_pointer(int a, long long b, int c) { return a * 3 + ((int *)&b)[1] * 5 + (int)b * 7 + c * 11; }
int through_union(int a, long long b, int c) { union U u; u.q = b; return a * 3 + u.w[1] * 5 + u.w[0] * 7 + c * 11; }
int on_the_stack(int a, int b, int c, int d, long long e) { return a + b + c + d + ((int *)&e)[1] * 3 + (int)e * 5; }
int __cdecl explicit_cdecl(int a, long long b) { return ((int *)&b)[1] * 2 + (int)b + a; }
int by_bytes(long long b) { unsigned char *p = (unsigned char *)&b; return (p[4] + (p[5] << 8)) * 5 + p[0] + (p[1] << 8); }

int main(void)
{
    long long b = 0x200000003LL;
    report(through_pointer(1, b, 4));
    report(through_union(1, b, 4));
    report(on_the_stack(1, 2, 3, 4, b));
    report(explicit_cdecl(5, b));
    report(by_bytes(b));
    return 0;
}
