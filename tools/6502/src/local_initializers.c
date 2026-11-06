// flags: -O0 | -O2 | -Os
// A local aggregate with a constant initializer (gcc.c-torture 20000412-5: `struct { int node, type; } g[1] = {{0, 1}}`): a flat
// unit's front end lays its data down under the code segment, and the unit was refused with ".X is neither defined nor
// imported".
extern void report(long value);

struct pair { int node; int type; };

static long sum(const int *values, int count)
{
    long total = 0;
    int i;
    for (i = 0; i < count; ++i)
        total = total * 7 + values[i];
    return total;
}

int main(void)
{
    struct pair one[1] = { { 0, 1 } };
    struct pair many[3] = { { 3, 4 }, { 5, 6 }, { 7, 8 } };
    int numbers[5] = { 11, 22, 33, 44, 55 };
    int partial[4] = { 9, 8 };
    long wide[2] = { 100000L, -200000L };

    report(one[0].node * 10 + one[0].type);
    report(many[0].node + many[1].type * 100L + many[2].node * 10000L);
    report(sum(numbers, 5));
    report(sum(partial, 4));
    report(wide[0] + wide[1]);
    return 0;
}
