// flags: -O0 | -O2 | -Os
// The call of a function whose body returns no value, used as a value (gcc.c-torture 20000717-1: `trio` aside, `bar (i, t) { }`
// and `return bar (i, t)`): the front end types the call as an int, the function was made a void one, and the use was
// refused with "a void call's value". The value is undefined; the program must compile and run on.
extern void report(long value);

int nothing(int i) { (void)i; }

long long nothing_wide(long long i) { (void)i; }

int through(int i) { return nothing(i); }

long long through_wide(long long i) { return nothing_wide(i); }

int main(void)
{
    int taken = nothing(3);
    (void)taken;
    (void)through(4);
    (void)through_wide(5LL);
    report(42);
    return 0;
}
