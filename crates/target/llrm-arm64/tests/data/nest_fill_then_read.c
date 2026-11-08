long f(int n)
{
    short t[8][8];
    long s = 0;
    int i, j, k;

    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) t[i][j] = (short)(((i + 1) * (j + 2) + (i ^ j)) % 7);
    for (k = 0; k < n; ++k) s += t[k % 8][(k / 8) % 8];
    return s;
}
