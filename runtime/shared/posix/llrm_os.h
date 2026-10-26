/* The OS layer's interface for a POSIX host (runtime/shared/interface.toml), rendered by
   llrm-target's os.rs; regenerate with LLRM_BLESS=1 cargo test -p llrm-target the_posix_header. */
#ifndef LLRM_OS_H
#define LLRM_OS_H

#include <stddef.h>
#include <stdint.h>

int llrm_os_open(const char *, unsigned char);
int llrm_os_create(const char *);
int32_t llrm_os_read(int, unsigned char *, size_t);
int32_t llrm_os_seek(int, int32_t, unsigned char);
int32_t llrm_os_write_file(int, const unsigned char *, size_t);
intptr_t llrm_os_close(int);
void llrm_os_exit(unsigned char);
unsigned char * llrm_os_more(size_t);
unsigned char llrm_os_console_read_key(void);
unsigned char llrm_os_console_key_ready(void);
intptr_t llrm_os_command_line(unsigned char *, size_t);
intptr_t llrm_os_environment(size_t, unsigned char *, size_t);
int32_t llrm_os_monotonic(void);
intptr_t llrm_os_local_time(unsigned char *);
void llrm_os_sleep(size_t);
size_t llrm_os_environment_read(size_t, unsigned char *, size_t);
intptr_t llrm_os_environment_write(size_t, const unsigned char *, size_t);
intptr_t llrm_os_execute(const char *, const char *);
intptr_t llrm_os_delete(const char *);
intptr_t llrm_os_rename(const char *, const char *);
intptr_t llrm_os_mkdir(const char *);
intptr_t llrm_os_chdir(const char *);
intptr_t llrm_os_rmdir(const char *);
intptr_t llrm_os_find_first(const char *, size_t, unsigned char *);
intptr_t llrm_os_find_next(unsigned char *);
int32_t llrm_os_free_bytes(size_t);
intptr_t llrm_os_current_directory(size_t, unsigned char *);
intptr_t llrm_os_current_drive(void);
#define LLRM_OS_DENIED 5
#define LLRM_OS_NOT_FOUND 2
#define LLRM_OS_PATH_NOT_FOUND 3
#define LLRM_OS_STDIN 0
#define LLRM_OS_STDOUT 1
#define LLRM_OS_STDERR 2

/* The program's arguments, which os.c answers command_line and environment from; called once by main. */
void llrm_os_set_arguments(int argc, char **argv);
/* What llrm_os_exit calls, with the code, instead of exit(): a host that runs the program on a thread stops its other threads here. */
void llrm_os_set_exit_hook(void (*hook)(int code));

#endif
