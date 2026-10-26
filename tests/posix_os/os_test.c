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

    /* the files ops: a directory made, searched with DOS's *.* and a DOS path, renamed, deleted, removed */
    {
        char dir[256], one[300], two[300], dos[300];
        unsigned char entry[43];
        unsigned char block[8];
        uint32_t size;
        int made;

        snprintf(dir, sizeof dir, "%s/os_test_dir", getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp");
        llrm_os_delete("os_test_dir/A.TXT");
        CHECK(llrm_os_mkdir(dir) == 0 || llrm_os_chdir(dir) == 0);
        CHECK(llrm_os_chdir(dir) == 0);
        made = llrm_os_create("A.TXT");
        CHECK(made >= 0 && llrm_os_write_file(made, (const unsigned char *)"12345", 5) == 5 && llrm_os_close(made) == 0);
        made = llrm_os_create("README");
        CHECK(made >= 0 && llrm_os_close(made) == 0);
        /* *.* names a file without an extension too; the entry holds the name at 30 and the size at 26 */
        CHECK(llrm_os_find_first("*.*", 0, entry) == 0);
        CHECK(strcmp((char *)entry + 30, "A.TXT") == 0);
        memcpy(&size, entry + 26, 4);
        CHECK(size == 5 && entry[21] == 0);
        CHECK(llrm_os_find_next(entry) == 0 && strcmp((char *)entry + 30, "README") == 0);
        CHECK(llrm_os_find_next(entry) < 0);
        CHECK(llrm_os_find_first("*.TXT", 0, entry) == 0 && llrm_os_find_next(entry) < 0);
        CHECK(llrm_os_find_first("NONE.*", 0, entry) < 0);
        snprintf(one, sizeof one, "%s", dir);
        snprintf(dos, sizeof dos, "C:%s\\A.TXT", dir);
        snprintf(two, sizeof two, "%s/B.TXT", dir);
        CHECK(llrm_os_rename(dos, two) == 0);
        CHECK(llrm_os_rename(two, "README") == -80);
        CHECK(llrm_os_open("A.TXT", 0) == -LLRM_OS_NOT_FOUND);
        CHECK(llrm_os_delete("B.TXT") == 0 && llrm_os_delete("README") == 0 && llrm_os_delete("B.TXT") < 0);
        CHECK(llrm_os_current_directory(0, buffer) == 0 && strstr((char *)buffer, "os_test_dir") != NULL);
        CHECK(strchr((char *)buffer, '/') == NULL && llrm_os_current_drive() == 2 && llrm_os_free_bytes(0) > 0);
        CHECK(llrm_os_chdir("..") == 0 && llrm_os_rmdir(dir) == 0 && llrm_os_rmdir(dir) < 0);
        /* the environment block: strings, then an empty one; a write is read back, and past the room is -1 */
        CHECK(llrm_os_environment_read(0, block, 8) == 8);
        CHECK(llrm_os_environment_write(0, (const unsigned char *)"QBX=1\0\0", 8) == 0);
        CHECK(llrm_os_environment_read(0, block, 8) == 8 && memcmp(block, "QBX=1\0\0", 8) == 0);
        CHECK(llrm_os_environment(0, buffer, sizeof buffer) == 5 && memcmp(buffer, "QBX=1", 5) == 0);
        CHECK(llrm_os_environment_write(1u << 20, block, 8) == -1);
        /* execute: COMMAND.COM's /C is the host shell, and the block is the child's environment */
        snprintf(one, sizeof one, "exit 0");
        CHECK(llrm_os_execute("COMMAND.COM", "\x09/C exit 0\r") == 0);
    }

    CHECK(llrm_os_more(1024) != NULL);
    puts(failures ? "failed" : "ok");
    return failures != 0;
}
