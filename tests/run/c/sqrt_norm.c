// flags: -O2 | -Os
// Vector lengths and distances in a loop: `sqrt` may set errno, so it is a call unless the target splits it into the
// instruction plus a call for the error path (PartiallyInlineLibCalls). Every argument is a perfect square: the output
// is the same with or without extended precision.
extern void report(long value);
double sqrt(double);
float sqrtf(float);

static const short vec[8][3] = {
    {1, 2, 2}, {2, 3, 6}, {1, 4, 8}, {4, 4, 7}, {2, 6, 9}, {6, 6, 7}, {3, 4, 12}, {2, 10, 11},
};

static double length(double x, double y, double z)
{
    return sqrt(x * x + y * y + z * z);
}

static float flat(float x, float y)
{
    return sqrtf(x * x + y * y);
}

int main(void)
{
    double total = 0;
    float plane = 0;
    short r, i;

    for (r = 1; r <= 50; ++r)
        for (i = 0; i < 8; ++i) {
            total += length(r * vec[i][0], r * vec[i][1], r * vec[i][2]);
            plane += flat(3 * r, 4 * r);
        }
    report((long)total);
    report((long)plane);
    return 0;
}
