void g(int, int);
void h(void);
int f(int a) {
    g(a, 1);
    h();
    return a;
}

/* Two calls of 8 bytes each: one `push esi` leaves the stack 8 past 16, so neither call nor the frame needs a pad. */
int pair(int a) {
    g(a, 1);
    g(a, 2);
    return a;
}

/* A four-byte local and a four-byte argument: eight bytes of frame, none per call. */
extern void sink(volatile int *);
int local_alone(void) {
    volatile int x = 1;
    sink(&x);
    return x;
}

/* Three calls of 4, 4 and 8 bytes: the cleanup of one is the pad of the next. */
void g1(int);
int trio(int a) {
    g1(a);
    g1(a + 1);
    g(a, 1);
    return a;
}
