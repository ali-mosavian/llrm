long reported;
void report(long v) { reported = v; }
void *memcpy(void *d, const void *s, unsigned long n) { char *a = d; const char *b = s; while (n--) *a++ = *b++; return d; }
void *memmove(void *d, const void *s, unsigned long n) { char *a = d; const char *b = s; if (a < b) while (n--) *a++ = *b++; else { a += n; b += n; while (n--) *--a = *--b; } return d; }
void *memset(void *d, int c, unsigned long n) { char *a = d; while (n--) *a++ = (char)c; return d; }
int memcmp(const void *x, const void *y, unsigned long n) { const unsigned char *a = x, *b = y; for (; n; n--, a++, b++) if (*a != *b) return *a - *b; return 0; }
/* The OS layer for the programs that read and write files: raw Linux syscalls, answered by count.py. */
static long sys4(long n, long a, long b, long c)
{
    register long x8 __asm__("x8") = n, x0 __asm__("x0") = a, x1 __asm__("x1") = b, x2 __asm__("x2") = c;
    __asm__ volatile("svc #0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    return x0;
}
int llrm_os_open(const char *name, unsigned char mode) { (void)mode; return (int)sys4(56, -100, (long)name, 0); }
int llrm_os_create(const char *name) { return (int)sys4(56, -100, (long)name, 0x241); }
/* A timed-only program reads the first CI_BYTES of its corpus, the text its .ci.out was made from: its bench.toml ci_bytes, which
   build2.sh passes (0: all of it). */
#ifndef CI_BYTES
#error "build2.sh passes CI_BYTES from the program's bench.toml"
#endif
int llrm_os_read(int h, unsigned char *b, unsigned long n)
{
    static long left = CI_BYTES;
    long got;
    if (CI_BYTES && (long)n > left) n = (unsigned long)left;
    got = sys4(63, h, (long)b, (long)n);
    if (CI_BYTES && got > 0) left -= got;
    return (int)got;
}
int llrm_os_write_file(int h, const unsigned char *b, unsigned long n) { return (int)sys4(64, h, (long)b, (long)n); }
long llrm_os_close(int h) { return sys4(57, h, 0, 0); }
unsigned char host_text_screen[4000];

/* grep's input: the dickens corpus, cut by llrm_os_read. */
int input_read(char *buffer, int count)
{
    static int fd = -2;
    if (fd == -2) fd = llrm_os_open("DICKENS", 0);
    if (fd < 0) return -1;
    return llrm_os_read(fd, (unsigned char *)buffer, (unsigned long)count);
}
