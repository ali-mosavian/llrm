// flags: -O0 | -O2 | -Os
// A double compared with an int (gcc.c-torture 930702-1 at -O0: `a != 33`): the int is converted to a double, and the
// comparison read the converted value's cell as a float, `fcomp dword ptr` of an integer's bits.
extern void report(long value);

static int classify(double a, int b)
{
    int bits = 0;
    if (a != 33) bits |= 1;
    if (a < b) bits |= 2;
    if (a > b) bits |= 4;
    if (b == 11) bits |= 8;
    if (a == (double)b) bits |= 16;
    return bits;
}

int main(void)
{
    report(classify(33.0, 11));
    report(classify(11.0, 11));
    report(classify(-5.5, 11));
    report(classify(12.0, -7));
    report(classify(33.0, 33));
    return 0;
}
