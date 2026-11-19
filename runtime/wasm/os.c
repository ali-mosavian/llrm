/* The OS layer on wasm32 (runtime/shared/interface.toml): what the node harness gives. Output and exit
   and the clock are imports of the module (`env`); memory is a bump heap in the module's own. */
#include <string.h>
#include <time.h>

#include "llrm_os.h"

extern void llrm_host_write(int handle, const unsigned char *data, size_t count);
extern void llrm_host_exit(int code);
/* Milliseconds since the host's start. */
extern double llrm_host_now(void);

#define HEAP_BYTES (80UL << 20)

static unsigned char heap[HEAP_BYTES] __attribute__((aligned(16)));
static size_t heap_used;

/* The host's files (nodefs.mjs through host.mjs): a result below zero is minus the interface's code. Handles 0 to 2
   are the console. */
extern int llrm_host_fs_open(const char *name, int mode);
extern int llrm_host_fs_create(const char *name);
extern int llrm_host_fs_read(int handle, unsigned char *data, size_t count);
extern int llrm_host_fs_write(int handle, const unsigned char *data, size_t count);
extern int llrm_host_fs_seek(int handle, int offset, int origin);
extern int llrm_host_fs_close(int handle);
extern int llrm_host_fs_remove(const char *name);
extern int llrm_host_fs_rename(const char *from, const char *to);
extern int llrm_host_fs_mkdir(const char *name);
extern int llrm_host_fs_chdir(const char *name);
extern int llrm_host_fs_rmdir(const char *name);
extern int llrm_host_fs_directory(unsigned char *data, size_t max);
extern int llrm_host_fs_free(void);
extern int llrm_host_fs_find(const char *pattern, size_t attrs, size_t index, unsigned char *entry);
extern int llrm_host_command_line(unsigned char *data, size_t max);
extern int llrm_host_environment(size_t index, unsigned char *data, size_t max);

enum { DIRECTORY_MAX = 64, ENVIRONMENT_SLACK = 1024, INT32_LIMIT = 0x7FFFFFFF };

static size_t clamp(size_t count)
{
    return count > INT32_LIMIT ? INT32_LIMIT : count;
}

int32_t llrm_os_write_file(int handle, const unsigned char *data, size_t count)
{
    if (handle > LLRM_OS_STDERR)
        return llrm_host_fs_write(handle, data, clamp(count));
    llrm_host_write(handle, data, count);
    return (int32_t)count;
}

int llrm_os_open(const char *name, unsigned char mode)
{
    return llrm_host_fs_open(name, mode);
}

int llrm_os_create(const char *name)
{
    return llrm_host_fs_create(name);
}

int32_t llrm_os_read(int handle, unsigned char *data, size_t count)
{
    return llrm_host_fs_read(handle, data, clamp(count));
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

intptr_t llrm_os_close(int handle)
{
    return llrm_host_fs_close(handle);
}

int32_t llrm_os_seek(int handle, int32_t position, unsigned char origin)
{
    return llrm_host_fs_seek(handle, position, origin);
}

intptr_t llrm_os_delete(const char *name)
{
    return llrm_host_fs_remove(name);
}

intptr_t llrm_os_rename(const char *old, const char *new_name)
{
    return llrm_host_fs_rename(old, new_name);
}

intptr_t llrm_os_mkdir(const char *name)
{
    return llrm_host_fs_mkdir(name);
}

intptr_t llrm_os_chdir(const char *name)
{
    return llrm_host_fs_chdir(name);
}

intptr_t llrm_os_rmdir(const char *name)
{
    return llrm_host_fs_rmdir(name);
}

intptr_t llrm_os_find_first(const char *pattern, size_t attrs, unsigned char *entry)
{
    return llrm_host_fs_find(pattern, attrs, 0, entry);
}

/* The entry's first four bytes count the matches handed out. */
intptr_t llrm_os_find_next(unsigned char *entry)
{
    unsigned index = entry[0] | entry[1] << 8 | entry[2] << 16 | (unsigned)entry[3] << 24;

    /* find_first keeps the search; the pattern is not needed after it. */
    return llrm_host_fs_find("", 0, index, entry);
}

int32_t llrm_os_free_bytes(size_t drive)
{
    (void)drive;
    return llrm_host_fs_free();
}

/* The directory without a leading backslash, as DOS gives it, NUL-ended. */
intptr_t llrm_os_current_directory(size_t drive, unsigned char *data)
{
    int length;

    (void)drive;
    length = llrm_host_fs_directory(data, DIRECTORY_MAX - 1);
    data[length] = 0;
    return 0;
}

/* The host has one drive; programs see it as C:. */
intptr_t llrm_os_current_drive(void)
{
    return 2;
}

intptr_t llrm_os_command_line(unsigned char *data, size_t max)
{
    return llrm_host_command_line(data, max);
}

/* The environment block DOS keeps: NAME=VALUE strings, each NUL-ended, and an empty one at the end, with room to
   grow into. Built when first asked for. */
static unsigned char block[8192];
static size_t block_size, block_capacity;
static int block_ready;

static void block_build(void)
{
    size_t length = 0;
    int got;

    if (block_ready)
        return;
    block_ready = 1;
    block_capacity = sizeof block;
    for (size_t index = 0; length + ENVIRONMENT_SLACK < block_capacity; index++) {
        got = llrm_host_environment(index, block + length, block_capacity - ENVIRONMENT_SLACK - length - 2);
        if (got < 0)
            break;
        length += (size_t)got;
        block[length++] = 0;
    }
    block_size = length + 1;
}

intptr_t llrm_os_environment(size_t index, unsigned char *data, size_t max)
{
    size_t offset = 0, length;

    block_build();
    for (size_t at = 0; at < index; at++) {
        if (!block[offset])
            return -1;
        offset += strlen((char *)block + offset) + 1;
    }
    if (!block[offset])
        return -1;
    length = strlen((char *)block + offset);
    if (length > max)
        length = max;
    memcpy(data, block + offset, length);
    return (intptr_t)length;
}

size_t llrm_os_environment_read(size_t offset, unsigned char *data, size_t max)
{
    block_build();
    if (offset >= block_size)
        return 0;
    if (max > block_size - offset)
        max = block_size - offset;
    memcpy(data, block + offset, max);
    return max;
}

intptr_t llrm_os_environment_write(size_t offset, const unsigned char *data, size_t count)
{
    block_build();
    if (offset + count > block_capacity)
        return -1;
    memcpy(block + offset, data, count);
    if (offset + count > block_size)
        block_size = offset + count;
    return 0;
}

/* No processes: SHELL and CHAIN are told they are denied. */
intptr_t llrm_os_execute(const char *name, const char *tail)
{
    (void)name;
    (void)tail;
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
