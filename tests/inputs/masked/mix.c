long mix(int n) {
    long t = 0;
    int i;
    for (i = 1; i != n; i++) t += (long)(i & 255);
    return t;
}
