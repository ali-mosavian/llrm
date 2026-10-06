// flags: -O2 -march=i486 | -Os -march=i486
/* A byte stored from a loop's index into a frame array: the index sat in si or di, which have no byte half, and
   the body was refused ("value may be in no register"). Here with a variable-length copy loop between two such arrays. */
extern void report(long value);

static int bench(int n)
{
    char a[70], b[70];
    int i;

    for (i = 0; i < 70; i++) a[i] = (char)i;
    for (i = 0; i < (n & 63); i++) b[i] = a[i];
    return b[3];
}

int main(void)
{
    report(bench(63) * 100 + bench(5));
    return 0;
}
