/* Arguments that are volatile loads are read in source order. Each global sits
   on its own PROT_NONE page; the fault handler logs the order the pages are
   touched and unprotects the page. -DREVERSED reverses the call: the order
   must follow the source, so the control prints 321, not 123. */
#include <signal.h>
#include <stdio.h>
#include <sys/mman.h>

static char *pages;
static int seen[8];
static int count;

static void touched(int sig, siginfo_t *info, void *context)
{
    (void)sig;
    (void)context;
    char *page = (char *)((unsigned long)info->si_addr & ~4095ul);
    if (count < 8) seen[count++] = (int)((page - pages) / 4096) + 1;
    mprotect(page, 4096, PROT_READ | PROT_WRITE);
}

static volatile int *g1, *g2, *g3;

__attribute__((noinline)) static int sink(int a, int b, int c) { return a * 100 + b * 10 + c; }

int main(void)
{
    pages = mmap(0, 3 * 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    for (int i = 0; i < 3; i++) *(volatile int *)(pages + i * 4096) = i + 1;
    for (int i = 0; i < 3; i++) mprotect(pages + i * 4096, 4096, PROT_NONE);
    struct sigaction action = { .sa_sigaction = touched, .sa_flags = SA_SIGINFO };
    sigaction(SIGSEGV, &action, 0);
    g1 = (volatile int *)pages;
    g2 = (volatile int *)(pages + 4096);
    g3 = (volatile int *)(pages + 8192);
#ifdef REVERSED
    int result = sink(*g3, *g2, *g1);
#else
    int result = sink(*g1, *g2, *g3);
#endif
    printf("args=%d order=%d%d%d\n", result, seen[0], seen[1], seen[2]);
    return 0;
}
