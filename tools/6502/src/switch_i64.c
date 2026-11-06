// flags: -O2 -march=i486 | -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* A switch on a long long (wider than a register): selected, it was refused ("a i64 value", gcc.c-torture/930111-1).
   The cases compare by halves; the default reads the selector too. */
extern void report(long value);

int classify(long long i)
{
    switch (i) {
    case 3:
    case 10:
        return 1;
    case 0x100000003LL:
        return 2;
    case -1:
        return 3;
    case 47:
        return 4;
    default:
        return (int)(i >> 32) * 100 + 5;
    }
}

int main(void)
{
    report(classify(3));
    report(classify(10));
    report(classify(0x100000003LL));
    report(classify(-1LL));
    report(classify(47));
    report(classify(0x200000003LL));
    report(classify(0));
    return 0;
}
