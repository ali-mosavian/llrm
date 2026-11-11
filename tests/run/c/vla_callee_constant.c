// flags: -O2
// targets: !m68k !wasm32 !z80-sdcc !6502-sim65 !65c02-sim65
/* Static functions with a variable-length array, called with constants and from a loop: inlined, the arrays are frame slots or bracketed by stacksave/stackrestore and no call is left (#8939 left the calls). */
extern void report(long value);

static int vla(int n)
{
    int a[n];
    int i, s = 0;

    for (i = 0; i < n; i++)
        a[i] = i * i;
    for (i = 0; i < n; i++)
        s += a[i];
    return s;
}

static long nested(int n)
{
    long total = 0;
    int k, i;

    for (k = 1; k <= n; k++) {
        char buffer[k * 3];

        for (i = 0; i < k * 3; i++)
            buffer[i] = (char)(i + k);
        total += buffer[k * 3 - 1] + vla(k);
    }
    return total;
}

int main(void)
{
    report(vla(5));
    report(vla(50));
    report(nested(6));
    return 0;
}
