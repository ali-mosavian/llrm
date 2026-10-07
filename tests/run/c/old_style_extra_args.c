// flags: -O2 -march=i486 -fno-inline-functions -fno-inline-functions-called-once | -O2 -march=i486 -m32 -fno-inline-functions -fno-inline-functions-called-once | -O2 -march=i486 -m32 -mabi=sysv -fno-inline-functions -fno-inline-functions-called-once
/* A function defined with () or fewer parameters than a call passes: what the callee does not declare is the caller's to remove,
   though the convention's callee pops its own (gcc.c-torture/20051012-1: the stack leaked the struct and main returned elsewhere). */
extern void report(long value);

struct three { long a, b, c; } t;

int counter;
int none() { counter++; return 42; }
int two() { counter += 2; return 5; }

/* Its frame is addressed from ESP, so a leaked argument shows in what it reads after the call. */
int framed(int x)
{
    volatile int v[4];
    v[0] = x; v[1] = x + 1; v[2] = x + 2; v[3] = x + 3;
    none(t, t);
    return v[0] + v[1] * 10 + v[2] * 100 + v[3] * 1000;
}

int main(void)
{
    int base;
    t.a = 1; t.b = 2; t.c = 3;
    report(none(t));
    report(none(1, 2, 3));
    base = none(7) + two(t, 9L, 1.5) + none();
    report(base);
    report(two(t) + base);
    report(framed(1));
    return 0;
}
