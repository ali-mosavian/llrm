long g7(long, long, long, long, long, long, long);

long one_call(long a) { return g7(a, 2, 3, 4, 5, 6, 7); }

long two_calls_and_locals(long a) {
    volatile long t[3];
    t[0] = a;
    return g7(a, 2, 3, 4, 5, 6, t[0]) + g7(1, a, 3, 4, 5, 6, 7);
}

long odd_locals(long a) {
    volatile char t[17];
    t[0] = a;
    return g7(a, 2, 3, 4, 5, 6, t[0]);
}
