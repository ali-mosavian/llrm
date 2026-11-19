unsigned char f(unsigned char a, unsigned char b, unsigned char m)
{
    unsigned char d = 0, e = 0;
    for (unsigned char i1 = 0; i1 < m; ++i1) {
        for (unsigned char i4 = 0; i4 < m; ++i4) {
            if (b > e) {
                for (unsigned char i5 = 0; i5 < (unsigned char)(m + 1); ++i5) {
                    a = 2;
                    d = 1;
                }
            }
        }
    }
    return (unsigned char)(a + b + d + e);
}
