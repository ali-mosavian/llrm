/* -Os: a one-slot frame (the float's store to the stack) in a function that restores esi. */
float V[200];

long slot_with_saved(int n)
{
    long s = 0;
    float *p = V;
    int i;
    for (i = 0; i < n; i++, p += 11) {
        float x = p[0] * p[1];
        union { float f; long l; } u;
        u.f = x;
        s += (x != x) ? 0x7e000000L : u.l * 5;
    }
    return s;
}

/* No register to restore: the slot is released at the ret, into a dead register. */
extern void sink(volatile int *);
long slot_alone(void)
{
    volatile int x = 1;
    sink(&x);
    return x;
}
