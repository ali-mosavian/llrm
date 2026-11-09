long reported;
void report(long v) { reported = v; }
void *memcpy(void *d, const void *s, unsigned long n) { char *a = d; const char *b = s; while (n--) *a++ = *b++; return d; }
void *memmove(void *d, const void *s, unsigned long n) { char *a = d; const char *b = s; if (a < b) while (n--) *a++ = *b++; else { a += n; b += n; while (n--) *--a = *--b; } return d; }
void *memset(void *d, int c, unsigned long n) { char *a = d; while (n--) *a++ = (char)c; return d; }
int memcmp(const void *x, const void *y, unsigned long n) { const unsigned char *a = x, *b = y; for (; n; n--, a++, b++) if (*a != *b) return *a - *b; return 0; }
/* grep's input: 64 KiB of fixed text (same bytes for every compiler). */
static const char grep_line[] = "Oliver met Scrooge and the ghost of Marley, a gentleman with Tiny Tim; Heep and Micawber dined at Dombey and Copperfield.\n";
int input_read(char *buffer, int count)
{
    static long done;
    int n = 0;
    while (n < count && done < 65536) { buffer[n++] = grep_line[done % (sizeof grep_line - 1)]; ++done; }
    return n;
}

/* The OS layer for the programs that read and write files: raw Linux syscalls, answered by count.py. */
static long sys4(long n, long a, long b, long c)
{
    register long x8 __asm__("x8") = n, x0 __asm__("x0") = a, x1 __asm__("x1") = b, x2 __asm__("x2") = c;
    __asm__ volatile("svc #0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    return x0;
}
int llrm_os_open(const char *name, unsigned char mode) { (void)mode; return (int)sys4(56, -100, (long)name, 0); }
int llrm_os_create(const char *name) { return (int)sys4(56, -100, (long)name, 0x241); }
int llrm_os_read(int h, unsigned char *b, unsigned long n) { return (int)sys4(63, h, (long)b, (long)n); }
int llrm_os_write_file(int h, const unsigned char *b, unsigned long n) { return (int)sys4(64, h, (long)b, (long)n); }
long llrm_os_close(int h) { return sys4(57, h, 0, 0); }
unsigned char host_text_screen[4000];
