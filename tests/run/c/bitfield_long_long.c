// flags: -O0 | -O2 -march=i486 | -O0 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* Bit fields of a long long unit read and write the bytes they cover: a packed struct of 5 bytes had its fields read and written
   as 8, past the struct, and a copy of it on the stack overwrote the frame (gcc.c-torture/930126-1 on -m16 at -O0). */
extern void report(long value);

struct s { unsigned long long a:8, b:32; };
struct t { unsigned long long lo:3, mid:50, hi:11; };

struct s f(struct s x)
{
    x.b = 0xcdef1234;
    return x;
}

int main(void)
{
    static struct s i;
    struct s j;
    struct t t;
    i.a = 12;
    i = f(i);
    report(i.a);
    report(i.b == 0xcdef1234UL);
    j.a = 200; j.b = 7;
    j.b += 5;
    report((long)j.a * 1000 + (long)j.b);
    t.lo = 5; t.mid = 0x123456789ULL; t.hi = 1000;
    report(t.lo);
    report((long)(t.mid & 0xFFFFFF));
    report(t.hi);
    return 0;
}
