// flags: -O0 | -O2 | -O3
// The twin of object_extent_loop with float for double: SDCC's double is 32 bits ("types double, long double not supported. Assuming float"), so this is the program its columns run.
// Float literals carry `f` as well: SDCC reads every `double`, constant or type, as a float.
// A loop of a run-time `n` trips over a 12 x 12 array is copied out by its extent at -O3 (each copy keeps its exit):
// every n from 0 to 12 must sum what the rolled loop does, the short ones leaving at their own test.
extern void report(long value);

static float A[12][12], B[12][12];

static float dot(int n, int i, int j)
{
    float sum = 0;
    int k;

    for (k = 0; k < n; k++)
        sum += A[i][k] * B[k][j];
    return sum;
}

int main(void)
{
    int i, j, n;

    for (i = 0; i < 12; i++)
        for (j = 0; j < 12; j++) {
            A[i][j] = (i + j) * 0.5f;
            B[i][j] = (i - j) * 0.25f + 1.0f;
        }
    for (n = 0; n <= 12; n++)
        report((long)(dot(n, 3, 5) * 16) + (long)(dot(n, 11, 0) * 16));
    return 0;
}
