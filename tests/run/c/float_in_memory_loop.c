// flags: -O3 | -O3 -march=i486 -m32 -mabi=sysv
// bar: not comparable (SDCC double is 32 bits); twin float_in_memory_loop_f32
extern void report(long value);

/* A two-block counted loop whose float state lives in memory: a float accumulated in memory over a counted loop. */
static double acc, step;

long sum_of_steps(int n)
{
    int i, r;
    long s = 0;
    for (r = 0; r < n; ++r) {
        acc = 0.0;
        step = 0.5;
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
