/* llrm_os_* over raw m68k-linux syscalls, for qemu-user sweeps. */
typedef unsigned long size_t;
static long sc(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
int llrm_os_open(const char *n, unsigned char m) { static const int f[] = {0, 1, 2}; return m > 2 ? -22 : (int)sc(5, (long)n, f[m], 0); }
int llrm_os_create(const char *n) { return (int)sc(5, (long)n, 2 | 0100 | 01000, 0666); }
int llrm_os_read(int h, unsigned char *d, size_t c) { return (int)sc(3, h, (long)d, (long)c); }
int llrm_os_write_file(int h, const unsigned char *d, size_t c) { return (int)sc(4, h, (long)d, (long)c); }
int llrm_os_seek(int h, int p, unsigned char o) { return o > 2 ? -22 : (int)sc(19, h, p, o); }
long llrm_os_close(int h) { return sc(6, h, 0, 0) < 0 ? -1 : 0; }
void llrm_os_exit(unsigned char c) { sc(1, c, 0, 0); for (;;) ; }
long llrm_os_delete(const char *n) { return sc(10, (long)n, 0, 0) < 0 ? -1 : 0; }
long llrm_os_rename(const char *a, const char *b) { return sc(38, (long)a, (long)b, 0) < 0 ? -1 : 0; }
long llrm_os_mkdir(const char *n) { return sc(39, (long)n, 0755, 0) < 0 ? -1 : 0; }
long llrm_os_rmdir(const char *n) { return sc(40, (long)n, 0, 0) < 0 ? -1 : 0; }
long llrm_os_chdir(const char *n) { return sc(12, (long)n, 0, 0) < 0 ? -1 : 0; }
long llrm_os_find_first(const char *p, size_t a, unsigned char *e) { return -1; }
long llrm_os_find_next(unsigned char *e) { return -1; }
int llrm_os_free_bytes(size_t d) { return -1; }
long llrm_os_current_directory(size_t d, unsigned char *p) { return -1; }
long llrm_os_current_drive(void) { return 0; }
unsigned char llrm_os_console_read_key(void) { return 26; }
unsigned char llrm_os_console_key_ready(void) { return 1; }
int input_read(char *b, int n) { return (int)sc(3, 0, (long)b, n); }
static void out(const char *s, int n) { sc(4, 1, (long)s, n); }
int printf(const char *f, ...)
{
    __builtin_va_list ap;
    __builtin_va_start(ap, f);
    for (; *f; f++) {
        char buf[16]; int n = 0; unsigned long v; int neg = 0, base = 10;
        if (*f != '%') { out(f, 1); continue; }
        f++;
        while (*f == 'l') f++;
        if (*f == '%') { out(f, 1); continue; }
        if (*f == 'c') { char c = (char)__builtin_va_arg(ap, int); out(&c, 1); continue; }
        if (*f == 's') { const char *s = __builtin_va_arg(ap, const char *); int k = 0; while (s[k]) k++; out(s, k); continue; }
        v = __builtin_va_arg(ap, unsigned long);
        if (*f == 'd' && (long)v < 0) { neg = 1; v = -v; }
        if (*f == 'x') base = 16;
        do { buf[sizeof buf - 1 - n++] = "0123456789abcdef"[v % base]; v /= base; } while (v);
        if (neg) buf[sizeof buf - 1 - n++] = '-';
        out(buf + sizeof buf - n, n);
    }
    __builtin_va_end(ap);
    return 0;
}
