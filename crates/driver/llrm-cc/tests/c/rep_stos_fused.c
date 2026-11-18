/* A copy and a constant fill in one loop: both leave as string operations. */
void use(unsigned char *p);
void fused(const unsigned char *pattern) {
    unsigned char a[700], b[700];
    for (int i = 0; i < 700; i++) a[i] = pattern[i], b[i] = 0xEE;
    use(a);
    use(b);
}
