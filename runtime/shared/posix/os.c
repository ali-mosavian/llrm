/* The OS layer on a POSIX host: runtime/shared/interface.toml implemented with the C library. The DOS
   targets' names are kept (write_file is write, more is alloc, command_line is args). A result below
   zero is minus the code the interface gives the condition, else minus errno. */
#define _POSIX_C_SOURCE 200809L
#include "llrm_os.h"

#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

extern char **environ;

enum { CTRL_Z = 26, INT32_LIMIT = 0x7FFFFFFF, TIME_WORDS = 7, MILLIS = 1000, NANOS_PER_MILLI = 1000000 };

static int program_argc;
static char **program_argv;
static void (*exit_hook)(int);

static int failure(void)
{
    switch (errno) {
    case ENOENT:
        return -LLRM_OS_NOT_FOUND;
    case ENOTDIR:
        return -LLRM_OS_PATH_NOT_FOUND;
    case EACCES:
    case EPERM:
        return -LLRM_OS_DENIED;
    default:
        return -errno;
    }
}

static size_t clamp(size_t count)
{
    return count > INT32_LIMIT ? INT32_LIMIT : count;
}

void llrm_os_set_arguments(int argc, char **argv)
{
    program_argc = argc;
    program_argv = argv;
}

void llrm_os_set_exit_hook(void (*hook)(int code))
{
    exit_hook = hook;
}

int llrm_os_open(const char *name, unsigned char mode)
{
    static const int flags[] = {O_RDONLY, O_WRONLY, O_RDWR};
    int fd;

    if (mode > 2)
        return -EINVAL;
    fd = open(name, flags[mode]);
    return fd < 0 ? failure() : fd;
}

int llrm_os_create(const char *name)
{
    int fd = open(name, O_RDWR | O_CREAT | O_TRUNC, 0666);

    return fd < 0 ? failure() : fd;
}

int32_t llrm_os_read(int handle, unsigned char *data, size_t count)
{
    ssize_t got = read(handle, data, clamp(count));

    return got < 0 ? failure() : (int32_t)got;
}

int32_t llrm_os_seek(int handle, int32_t position, unsigned char origin)
{
    static const int whence[] = {SEEK_SET, SEEK_CUR, SEEK_END};
    off_t at;

    if (origin > 2)
        return -EINVAL;
    at = lseek(handle, position, whence[origin]);
    return at < 0 ? failure() : (int32_t)at;
}

int32_t llrm_os_write_file(int handle, const unsigned char *data, size_t count)
{
    ssize_t put = write(handle, data, clamp(count));

    return put < 0 ? failure() : (int32_t)put;
}

intptr_t llrm_os_close(int handle)
{
    return close(handle) < 0 ? failure() : 0;
}

void llrm_os_exit(unsigned char code)
{
    if (exit_hook)
        exit_hook(code);
    exit(code);
}

unsigned char *llrm_os_more(size_t bytes)
{
    return malloc(bytes);
}

/* At the end of redirected input a key is Ctrl-Z, again and again, and always ready. */
unsigned char llrm_os_console_read_key(void)
{
    unsigned char key;

    return read(STDIN_FILENO, &key, 1) == 1 ? key : CTRL_Z;
}

unsigned char llrm_os_console_key_ready(void)
{
    struct pollfd waiting = {STDIN_FILENO, POLLIN, 0};

    return poll(&waiting, 1, 0) != 0;
}

/* The arguments after the program's name, a blank apart, cut at `max`. */
intptr_t llrm_os_command_line(unsigned char *data, size_t max)
{
    size_t length = 0;
    int at;

    for (at = 1; at < program_argc; at++) {
        const char *word = program_argv[at];

        if (at > 1 && length < max)
            data[length++] = ' ';
        while (*word && length < max)
            data[length++] = (unsigned char)*word++;
    }
    return (intptr_t)length;
}

intptr_t llrm_os_environment(size_t index, unsigned char *data, size_t max)
{
    size_t length;
    size_t at;

    for (at = 0; at <= index; at++)
        if (!environ || !environ[at])
            return -1;
    length = strlen(environ[index]);
    if (length > max)
        length = max;
    memcpy(data, environ[index], length);
    return (intptr_t)length;
}

int32_t llrm_os_monotonic(void)
{
    struct timespec now;

    clock_gettime(CLOCK_MONOTONIC, &now);
    return (int32_t)(((long long)now.tv_sec * MILLIS + now.tv_nsec / NANOS_PER_MILLI) & INT32_LIMIT);
}

intptr_t llrm_os_local_time(unsigned char *data)
{
    struct timespec now;
    struct tm local;
    unsigned words[TIME_WORDS];
    int at;

    clock_gettime(CLOCK_REALTIME, &now);
    if (!localtime_r(&now.tv_sec, &local))
        return failure();
    words[0] = (unsigned)local.tm_year + 1900;
    words[1] = (unsigned)local.tm_mon + 1;
    words[2] = (unsigned)local.tm_mday;
    words[3] = (unsigned)local.tm_hour;
    words[4] = (unsigned)local.tm_min;
    words[5] = (unsigned)local.tm_sec;
    words[6] = (unsigned)(now.tv_nsec / (NANOS_PER_MILLI * 10));
    for (at = 0; at < TIME_WORDS; at++) {
        data[2 * at] = (unsigned char)(words[at] & 0xFF);
        data[2 * at + 1] = (unsigned char)(words[at] >> 8);
    }
    return 0;
}

void llrm_os_sleep(size_t milliseconds)
{
    struct timespec wait = {(time_t)(milliseconds / MILLIS), (long)(milliseconds % MILLIS) * NANOS_PER_MILLI};

    nanosleep(&wait, 0);
}
