/* Ten floats rotate in a loop: a 10-wide cycle of spilled phi copies (#358). */
extern void report(long value);
long rot(short n, float seed)
{
    float a0 = seed, a1 = seed + 1, a2 = seed + 2, a3 = seed + 3, a4 = seed + 4;
    float a5 = seed + 5, a6 = seed + 6, a7 = seed + 7, a8 = seed + 8, a9 = seed + 9;
    float t;
    short i;
    for (i = 0; i < n; ++i) {
        t = a0;
        a0 = a1; a1 = a2; a2 = a3; a3 = a4; a4 = a5;
        a5 = a6; a6 = a7; a7 = a8; a8 = a9; a9 = t;
        a0 = a0 + a9 * 0.5f;
    }
    return (long)(a0 + 2 * a1 + 3 * a2 + 4 * a3 + 5 * a4 + 6 * a5 + 7 * a6 + 8 * a7 + 9 * a8 + 10 * a9);
}
int main(void) { report(rot(7, 1.0f)); return 0; }
