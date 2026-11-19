/* One PROT_NONE page per global, a SIGSEGV handler that logs the faulting page and opens it: the fault log is the
   order the volatile loads ran in. Exit 0 when it matches the order the source wrote (g1,g2,g3, or g3,g2,g1 with -DREV). */
typedef unsigned long u32;
static long sys(long n, long a, long b, long c, long d, long e, long f) {
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    register long d4 __asm__("d4") = d, d5 __asm__("d5") = e, a0 __asm__("a0") = f;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3), "d"(d4), "d"(d5), "a"(a0) : "memory");
    return d0;
}
static char *pages;
static long seen[8];
static int count;
struct ksa { void (*handler)(int, long *, void *); u32 flags; void *restorer; u32 mask[2]; };
static void handler(int signal, long *info, void *context) {
    long address = info[3];
    (void)signal; (void)context;
    if (count < 8) seen[count++] = address;
    sys(125, address & ~4095L, 4096, 3, 0, 0, 0);
}
volatile int *g1, *g2, *g3;
int probe(void);
__attribute__((noinline)) int sink(int a, int b, int c) { return a * 100 + b * 10 + c; }
int main(void) {
    pages = (char *)sys(192, 0, 3 * 4096, 3, 0x22, -1, 0);
    for (int i = 0; i < 3; i++) *(volatile int *)(pages + i * 4096) = i + 1;
    for (int i = 0; i < 3; i++) sys(125, (long)(pages + i * 4096), 4096, 0, 0, 0, 0);
    struct ksa action = { handler, 4, 0, { 0, 0 } };
    sys(174, 11, (long)&action, 0, 8, 0, 0);
    g1 = (volatile int *)pages; g2 = (volatile int *)(pages + 4096); g3 = (volatile int *)(pages + 8192);
    probe();
#ifdef REV
    return count == 3 && seen[0] == (long)g3 && seen[1] == (long)g2 && seen[2] == (long)g1 ? 0 : 1;
#else
    return count == 3 && seen[0] == (long)g1 && seen[1] == (long)g2 && seen[2] == (long)g3 ? 0 : 1;
#endif
}
void _start(void) { sys(1, main(), 0, 0, 0, 0, 0); for (;;) ; }
