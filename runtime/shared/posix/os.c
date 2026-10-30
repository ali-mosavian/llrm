/* The OS layer on a POSIX host: runtime/shared/interface.toml implemented with the C library. The DOS
   targets' names are kept (write_file is write, more is alloc, command_line is args). A result below
   zero is minus the code the interface gives the condition, else minus errno. */
#define _POSIX_C_SOURCE 200809L
#include "llrm_os.h"

#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <fnmatch.h>
#include <limits.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

extern char **environ;

enum {
    ENTRY_ATTR = 21,
    ENTRY_SIZE = 26,
    ENTRY_NAME = 30,
    ENTRY_NAME_MAX = 12,
    ATTR_SUBDIR = 0x10,
    DOS_NO_MORE_FILES = 18,
    DOS_NOT_SAME_DEVICE = 17,
    DOS_FILE_EXISTS = 80,
    DIRECTORY_MAX = 64,
    ENVIRONMENT_SLACK = 1024,
    COMMAND_TAIL_SWITCH = 3
};
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

static void block_build(void);
static unsigned char *block;

intptr_t llrm_os_environment(size_t index, unsigned char *data, size_t max)
{
    size_t length;
    size_t at, offset = 0;

    block_build();
    for (at = 0; at < index; at++) {
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

/* A DOS path as the host names it: no drive letter, a slash for each backslash. */
static const char *native(const char *path, char *out)
{
    size_t at = 0;

    if (path[0] && path[1] == ':')
        path += 2;
    for (; *path && at < PATH_MAX - 1; path++)
        out[at++] = *path == '\\' ? '/' : *path;
    out[at] = 0;
    return out;
}

intptr_t llrm_os_delete(const char *name)
{
    char path[PATH_MAX];

    return unlink(native(name, path)) ? failure() : 0;
}

intptr_t llrm_os_rename(const char *old, const char *new_name)
{
    char from[PATH_MAX], to[PATH_MAX];

    native(old, from);
    native(new_name, to);
    if (access(to, F_OK) == 0)
        return -DOS_FILE_EXISTS;
    if (rename(from, to) == 0)
        return 0;
    return errno == EXDEV ? -DOS_NOT_SAME_DEVICE : failure();
}

intptr_t llrm_os_mkdir(const char *name)
{
    char path[PATH_MAX];

    return mkdir(native(name, path), 0777) ? failure() : 0;
}

intptr_t llrm_os_chdir(const char *name)
{
    char path[PATH_MAX];

    return chdir(native(name, path)) ? failure() : 0;
}

intptr_t llrm_os_rmdir(const char *name)
{
    char path[PATH_MAX];

    return rmdir(native(name, path)) ? failure() : 0;
}

/* One search at a time, as DOS's: find_first lists the matches (by name, files and, when asked, directories whose
   names fit the entry), the entry's first four bytes count how many find_next has handed out. */
typedef char EntryName[ENTRY_NAME_MAX + 1];
static EntryName *matches;
static size_t match_count;
static char match_directory[PATH_MAX];

static int by_name(const void *one, const void *other)
{
    return strcmp(one, other);
}

static intptr_t entry_of(size_t index, unsigned char *entry)
{
    char path[PATH_MAX + ENTRY_NAME_MAX + 2];
    struct stat info;
    uint32_t size;

    while (index < match_count) {
        snprintf(path, sizeof path, "%s/%s", match_directory, matches[index]);
        if (stat(path, &info) == 0)
            break;
        index++;
    }
    if (index >= match_count)
        return -DOS_NO_MORE_FILES;
    size = S_ISDIR(info.st_mode) ? 0 : (uint32_t)(info.st_size > INT32_LIMIT ? INT32_LIMIT : info.st_size);
    memset(entry, 0, ENTRY_NAME + ENTRY_NAME_MAX + 1);
    entry[ENTRY_ATTR] = S_ISDIR(info.st_mode) ? ATTR_SUBDIR : 0;
    memcpy(entry + ENTRY_SIZE, &size, sizeof size);
    strcpy((char *)entry + ENTRY_NAME, matches[index]);
    index++;
    memcpy(entry, &index, sizeof index > 4 ? 4 : sizeof index);
    return 0;
}

intptr_t llrm_os_find_first(const char *pattern, size_t attrs, unsigned char *entry)
{
    char path[PATH_MAX];
    char *slash, *name;
    DIR *directory;
    struct dirent *one;
    size_t capacity = 0;
    struct stat info;

    native(pattern, path);
    slash = strrchr(path, '/');
    name = slash ? slash + 1 : path;
    if (slash)
        *slash = 0;
    snprintf(match_directory, sizeof match_directory, "%s", slash ? (slash == path ? "/" : path) : ".");
    /* DOS's *.* is every name, with an extension or not */
    if (strcmp(name, "*.*") == 0)
        name = "*";
    free(matches);
    matches = 0;
    match_count = 0;
    directory = opendir(match_directory);
    if (!directory)
        return failure();
    while ((one = readdir(directory)) != 0) {
        char full[PATH_MAX + 256];

        if (one->d_name[0] == '.' || strlen(one->d_name) > ENTRY_NAME_MAX || fnmatch(name, one->d_name, 0) != 0)
            continue;
        snprintf(full, sizeof full, "%s/%s", match_directory, one->d_name);
        if (stat(full, &info) != 0 || (S_ISDIR(info.st_mode) && !(attrs & ATTR_SUBDIR)))
            continue;
        if (match_count == capacity) {
            capacity = capacity ? capacity * 2 : 16;
            matches = realloc(matches, capacity * sizeof *matches);
            if (!matches) {
                closedir(directory);
                return -ENOMEM;
            }
        }
        strcpy(matches[match_count++], one->d_name);
    }
    closedir(directory);
    if (match_count)
        qsort(matches, match_count, sizeof *matches, by_name);
    return entry_of(0, entry) ? -LLRM_OS_NOT_FOUND : 0;
}

intptr_t llrm_os_find_next(unsigned char *entry)
{
    uint32_t index = 0;

    memcpy(&index, entry, sizeof index);
    return entry_of(index, entry);
}

int32_t llrm_os_free_bytes(size_t drive)
{
    struct statvfs space;
    unsigned long long bytes;

    (void)drive;
    if (statvfs(".", &space) != 0)
        return failure();
    bytes = (unsigned long long)space.f_bavail * space.f_frsize;
    return bytes > INT32_LIMIT ? INT32_LIMIT : (int32_t)bytes;
}

intptr_t llrm_os_current_directory(size_t drive, unsigned char *data)
{
    char path[PATH_MAX];
    size_t at, length = 0;

    (void)drive;
    if (!getcwd(path, sizeof path))
        return failure();
    for (at = path[0] == '/'; path[at] && length < DIRECTORY_MAX - 1; at++)
        data[length++] = (unsigned char)(path[at] == '/' ? '\\' : path[at]);
    data[length] = 0;
    return 0;
}

/* The host has one drive; programs see it as C:. */
intptr_t llrm_os_current_drive(void)
{
    return 2;
}

/* The environment block DOS keeps: NAME=VALUE strings, each NUL-ended, and an empty one at the end, with room to
   grow into. Built when first asked for; llrm_os_environment and execute read it from then on. */
static size_t block_size, block_capacity;

static void block_build(void)
{
    size_t at, length = 0;

    if (block)
        return;
    for (at = 0; environ && environ[at]; at++)
        length += strlen(environ[at]) + 1;
    block_capacity = length + 1 + ENVIRONMENT_SLACK;
    block = calloc(block_capacity, 1);
    if (!block)
        abort();
    for (at = 0, length = 0; environ && environ[at]; at++) {
        strcpy((char *)block + length, environ[at]);
        length += strlen(environ[at]) + 1;
    }
    block_size = length + 1;
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

static int is_shell(const char *name)
{
    const char *base = strrchr(name, '\\');

    base = base ? base + 1 : name;
    return strcasecmp(base, "COMMAND.COM") == 0 || strcasecmp(base, "CMD.EXE") == 0;
}

/* The program `name` with the tail DOS gives it (a length byte, the text, a carriage return), run to its end with
   the block as its environment. COMMAND.COM, which a host has not, is the shell: `/C text` is `sh -c text`. */
intptr_t llrm_os_execute(const char *name, const char *tail)
{
    char text[256], path[PATH_MAX], command[PATH_MAX + 300];
    char *arguments[] = {"sh", "-c", command, 0}, **variables;
    size_t length = (unsigned char)tail[0], count = 0, at;
    pid_t child;
    int status;

    if (length > 126)
        length = 126;
    memcpy(text, tail + 1, length);
    text[length] = 0;
    if (is_shell(name) || access(native(name, path), X_OK) != 0) {
        if (strncmp(text, "/C ", COMMAND_TAIL_SWITCH) == 0 || strncmp(text, "/c ", COMMAND_TAIL_SWITCH) == 0)
            snprintf(command, sizeof command, "%s", text + COMMAND_TAIL_SWITCH);
        else
            snprintf(command, sizeof command, "%s", getenv("SHELL") ? "exec \"$SHELL\"" : "exec sh");
    } else {
        snprintf(command, sizeof command, "exec '%s' %s", path, text);
    }
    block_build();
    for (at = 0; block[at];) {
        count++;
        at += strlen((char *)block + at) + 1;
    }
    variables = calloc(count + 1, sizeof *variables);
    if (!variables)
        return -ENOMEM;
    for (at = 0, count = 0; block[at]; count++)
        variables[count] = (char *)block + at, at += strlen((char *)block + at) + 1;
    child = fork();
    if (child < 0) {
        free(variables);
        return failure();
    }
    if (child == 0) {
        execve("/bin/sh", arguments, variables);
        _exit(127);
    }
    free(variables);
    while (waitpid(child, &status, 0) < 0 && errno == EINTR)
        ;
    return 0;
}
