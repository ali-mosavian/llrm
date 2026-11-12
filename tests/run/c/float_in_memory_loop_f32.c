// flags: -O3 | -O3 -march=i486 -m32 -mabi=sysv
// The twin of float_in_memory_loop with float for double: SDCC's double is 32 bits ("types double, long double not supported. Assuming float"), so this is the program its columns run.
// Float literals carry `f` as well: SDCC reads every `double`, constant or type, as a float.
extern void report(long value);

/* A two-block counted loop whose float state lives in memory: a float accumulated in memory over a counted loop. */
static float acc, step;

long sum_of_steps(int n)
{
    int i, r;
    long s = 0;
    for (r = 0; r < n; ++r) {
        acc = 0.0f;
        step = 0.5f;
        for (i = 0; i < 16; ++i) acc += step;
        s += (long)acc + r;
    }
    return s;
}

int main(void)
{
    report(sum_of_steps(100));
    return 0;
}
