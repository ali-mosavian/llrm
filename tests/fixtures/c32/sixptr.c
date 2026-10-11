/* Six arrays summed in a loop, every register taken: the frame register
   holds an array's start, and its address is made with lea. */
short __cdecl six(short *p0, short *p1, short *p2, short *p3, short *p4, short *p5, int n)
{
    short t = 0;
    int i;

    for (i = 0; i < n; i++) {
        t += p0[i];
        t += p1[i];
        t += p2[i];
        t += p3[i];
        t += p4[i];
        t += p5[i];
    }
    return t;
}
