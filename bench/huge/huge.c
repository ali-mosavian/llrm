/* 200x201 short array (80400 bytes) in __huge memory; Nib has no huge arrays. */
extern void report(long value);

short __huge h[200][201];

long bench_huge(void)
{
    long t = 0;
    short r, c;

    for (r = 0; r < 200; ++r)
        for (c = 0; c < 201; ++c)
            h[r][c] = (short)((r * 201L + c) % 251);
    for (r = 0; r < 200; ++r)
        for (c = 0; c < 201; ++c)
            t += h[r][c];
    return t;
}

int main(void)
{
    report(bench_huge());
    return 0;
}
