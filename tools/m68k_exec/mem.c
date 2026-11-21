/* Long-wise like a platform libc's: a long access needs an even address on the 68000, so a fill aligns by one byte and copies move
   longs only when both addresses have the same parity. Byte loops here priced every memset and memcpy call at four times a real one. */
typedef unsigned long ul;
void *memset(void *p, int c, unsigned long n){
    unsigned char *q = p, b = (unsigned char)c;
    if (n >= 8) {
        ul w = b * 0x01010101UL, *l;
        if ((ul)q & 1) { *q++ = b; n--; }
        for (l = (ul *)q, q += n & ~3UL; l != (ul *)q; ) *l++ = w;
        n &= 3;
    }
    while (n--) *q++ = b;
    return p;
}
void *memcpy(void *d, const void *s, unsigned long n){
    unsigned char *a = d; const unsigned char *b = s;
    if (n >= 8 && !(((ul)a ^ (ul)b) & 1)) {
        ul *x, *y;
        if ((ul)a & 1) { *a++ = *b++; n--; }
        for (x = (ul *)a, y = (ul *)b, a += n & ~3UL, b += n & ~3UL; x != (ul *)a; ) *x++ = *y++;
        n &= 3;
    }
    while (n--) *a++ = *b++;
    return d;
}
void *memmove(void *d, const void *s, unsigned long n){
    unsigned char *a = d; const unsigned char *b = s;
    if (a <= b || a >= b + n) return memcpy(d, s, n);
    a += n; b += n;
    if (n >= 8 && !(((ul)a ^ (ul)b) & 1)) {
        ul *x, *y;
        if ((ul)a & 1) { *--a = *--b; n--; }
        for (x = (ul *)a, y = (ul *)b, a -= n & ~3UL, b -= n & ~3UL; x != (ul *)a; ) *--x = *--y;
        n &= 3;
    }
    while (n--) *--a = *--b;
    return d;
}
int memcmp(const void *x, const void *y, unsigned long n){ const unsigned char *a=x,*b=y; for(;n;n--,a++,b++) if(*a!=*b) return *a-*b; return 0; }
