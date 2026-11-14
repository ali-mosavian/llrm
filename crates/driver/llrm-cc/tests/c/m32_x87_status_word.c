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

unsigned V[1024][2];

/* The six terms, then a conversion made only when its operand is in range
   (sfm16_float): the branch of the conversion must not part the terms from
   their sum. */
unsigned six_then_convert(int n)
{
    unsigned acc = 0;
    for (int i = 0; i < n; i++) {
        union { unsigned u; float f; } x = { V[i][0] }, y = { V[i][1] };
        float a = x.f, b = y.f;
        acc += (a < b) + 2 * (a == b) + 4 * (a > b) + 8 * (a != b) + 16 * (a <= b) + 32 * (a >= b);
        if (((x.u >> 23) & 0xFF) < 127 + 30) acc += (int)a;
    }
    return acc;
}
