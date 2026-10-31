int k(unsigned v, char *buf) {
    char *p = buf + 8;
    *p = 0;
    do { int d = v & 15; *--p = d < 10 ? '0' + d : 'a' + d - 10; v >>= 4; } while (v);
    return p[0] + p[3] + (int)(p - buf);
}
