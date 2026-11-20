extern void *memset(void *, int, unsigned long);
extern void *memmove(void *, const void *, unsigned long);

static void justify(unsigned char *dst, unsigned len, const unsigned char *src, unsigned count, _Bool right)
{
    unsigned n = count < len ? count : len;
    unsigned before = right ? len - n : 0;
    memset(dst, ' ', before);
    memmove(dst + before, src, n);
    memset(dst + before + n, ' ', len - n - before);
}

void lset(unsigned char *d, unsigned n, const unsigned char *s, unsigned c, _Bool r) { justify(d, n, s, c, r); }
void rset(unsigned char *d, unsigned n, const unsigned char *s, unsigned c, _Bool r) { justify(d + 1, n, s, c, r); }
void mset(unsigned char *d, unsigned n, const unsigned char *s, unsigned c, _Bool r) { justify(d + 2, n, s, c, r); }
