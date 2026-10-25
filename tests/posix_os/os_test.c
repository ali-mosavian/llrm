/* The POSIX OS layer (runtime/shared/posix/os.c) through the interface's own header: files, errors,
   arguments, environment, the keys at the end of input, the clock. Exits 0, or 1 naming the check. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "llrm_os.h"

static int failures;

#define CHECK(condition)                                                                       \
    do {                                                                                       \
        if (!(condition)) {                                                                    \
            printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #condition);                         \
            failures++;                                                                        \
        }                                                                                      \
    } while (0)

int main(int argc, char **argv)
{
    char path[256];
    unsigned char buffer[64], time[14];
    int handle;
    int32_t before, after;
    intptr_t length;

    llrm_os_set_arguments(argc, argv);
    snprintf(path, sizeof path, "%s/os_test.dat", getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp");

    handle = llrm_os_create(path);
    CHECK(handle >= 0);
    CHECK(llrm_os_write_file(handle, (const unsigned char *)"hello, world", 12) == 12);
    CHECK(llrm_os_seek(handle, 7, 0) == 7);
    CHECK(llrm_os_read(handle, buffer, 5) == 5 && memcmp(buffer, "world", 5) == 0);
    CHECK(llrm_os_seek(handle, -5, 2) == 7);
    CHECK(llrm_os_seek(handle, 0, 1) == 7);
    CHECK(llrm_os_close(handle) == 0);
    handle = llrm_os_open(path, 0);
    CHECK(handle >= 0 && llrm_os_read(handle, buffer, 64) == 12 && llrm_os_read(handle, buffer, 64) == 0);
    llrm_os_close(handle);
    CHECK(llrm_os_open("/nonexistent/dir/file", 0) == -LLRM_OS_PATH_NOT_FOUND || llrm_os_open("/nonexistent/dir/file", 0) == -LLRM_OS_NOT_FOUND);
    CHECK(llrm_os_open("/nonexistent-file-here", 0) == -LLRM_OS_NOT_FOUND);
    CHECK(llrm_os_open(path, 9) < 0);
    remove(path);

    length = llrm_os_command_line(buffer, sizeof buffer);
    CHECK(length == 7 && memcmp(buffer, "one two", 7) == 0);
    CHECK(llrm_os_command_line(buffer, 4) == 4 && memcmp(buffer, "one ", 4) == 0);
    CHECK(llrm_os_environment(0, buffer, sizeof buffer) > 0);
    CHECK(llrm_os_environment(100000, buffer, sizeof buffer) == -1);

    /* stdin is empty or closed: ready, and Ctrl-Z, again and again */
    CHECK(llrm_os_console_key_ready());
    CHECK(llrm_os_console_read_key() == 26 && llrm_os_console_read_key() == 26);

    before = llrm_os_monotonic();
    llrm_os_sleep(30);
    after = llrm_os_monotonic();
    CHECK(after - before >= 25 && after - before < 500);
    CHECK(llrm_os_local_time(time) == 0);
    CHECK((time[0] | time[1] << 8) >= 2024 && time[2] >= 1 && time[2] <= 12 && time[4] >= 1 && time[4] <= 31);

    CHECK(llrm_os_more(1024) != NULL);
    puts(failures ? "failed" : "ok");
    return failures != 0;
}
