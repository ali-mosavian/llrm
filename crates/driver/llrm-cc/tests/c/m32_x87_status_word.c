float A[1024], B[1024];

unsigned six(int n)
{
    unsigned acc = 0;
    for (int i = 0; i < n; i++) {
        float a = A[i], b = B[i];
        acc += (a < b) + 2 * (a == b) + 4 * (a > b) + 8 * (a != b) + 16 * (a <= b) + 32 * (a >= b);
    }
    return acc;
}

int equal(float a, float b) { return a == b; }
