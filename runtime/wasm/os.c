/* The OS layer on wasm32 (runtime/shared/interface.toml): what the node harness gives. Output and exit
   and the clock are imports of the module (`env`); memory is a bump heap in the module's own. */
#include <time.h>

#include "llrm_os.h"

extern void llrm_host_write(int handle, const unsigned char *data, size_t count);
extern void llrm_host_exit(int code);
/* Milliseconds since the host's start. */
extern double llrm_host_now(void);

#define HEAP_BYTES (80UL << 20)

static unsigned char heap[HEAP_BYTES] __attribute__((aligned(16)));
static size_t heap_used;

int32_t llrm_os_write_file(int handle, const unsigned char *data, size_t count)
{
    llrm_host_write(handle, data, count);
    return (int32_t)count;
}

void llrm_os_exit(unsigned char code)
{
    llrm_host_exit(code);
    for (;;)
        ;
}

unsigned char *llrm_os_more(size_t bytes)
{
    unsigned char *room;

    bytes = (bytes + 15) & ~(size_t)15;
    if (bytes > HEAP_BYTES - heap_used)
        return 0;
    room = heap + heap_used;
    heap_used += bytes;
    return room;
}

/* No files: a program that opens one is told it is denied. */
intptr_t llrm_os_close(int handle)
{
    (void)handle;
    return -LLRM_OS_DENIED;
}

int32_t llrm_os_seek(int handle, int32_t position, unsigned char origin)
{
    (void)handle;
    (void)position;
    (void)origin;
    return -LLRM_OS_DENIED;
}

/* The keyboard poll and the clock are where a program that waits for them spins: the host moves its virtual clock on
   each, so a wait ends, a key scheduled for a time arrives and a run that does not finish is stopped. Linked with
   `--wrap`. */
extern void llrm_host_poll(void);
/* The program looked for a key, and found `ready` of them: with none the host may type the next of a script. */
extern void llrm_host_key_poll(int ready);
extern int __real_dev_key_ready(void);
extern long __real_dev_clock(void);

/* A tone the program starts or ends (0): the host sounds it. */
extern void llrm_host_tone(unsigned hertz);
extern void __real_dev_tone(unsigned hertz);

void __wrap_dev_tone(unsigned hertz)
{
    llrm_host_tone(hertz);
    __real_dev_tone(hertz);
}

int __wrap_dev_key_ready(void)
{
    int ready;

    llrm_host_poll();
    ready = __real_dev_key_ready();
    llrm_host_key_poll(ready);
    return ready;
}

long __wrap_dev_clock(void)
{
    llrm_host_poll();
    return __real_dev_clock();
}

int clock_gettime(int clock, struct timespec *now)
{
    double ms = llrm_host_now();

    (void)clock;
    now->tv_sec = (time_t)(ms / 1000);
    now->tv_nsec = (long)((ms - (double)now->tv_sec * 1000) * 1000000);
    return 0;
}
