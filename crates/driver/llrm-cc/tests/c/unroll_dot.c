/* A dot product of a run-time length: the pin's `UP.Runtime` unrolls it, clang 20 and gcc do. RU-REC (a deviation, retired
   11-17) kept it rolled because its fadd chain bounds a trip; unrolling still saves the counter, compare and branch of
   every other trip (object_extent_loop's `dot` loops, 7 -> 5.5 instructions an element, +1.8% over clang). */
double A[1024], B[1024];

double dot(int n)
{
    double s = 0;
    for (int k = 0; k < n; k++)
        s += A[k] * B[k];
    return s;
}
