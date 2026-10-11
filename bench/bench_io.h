/* Files by name for the benches: llrm's OS layer under llrm-c, which predefines GCC's macros (__INT_MAX__) as the
   reference compilers do not, and their own <io.h> calls otherwise. Binary, in the current directory. A handle
   is below zero on an error; bench_read is 0 at the end of the file. */
#ifndef BENCH_IO_H
#define BENCH_IO_H

#ifdef __INT_MAX__
#include "llrm_os.h"
#define bench_open_read(name) ((int)llrm_os_open(name, 0))
#define bench_create(name) ((int)llrm_os_create(name))
#define bench_read(handle, buffer, count) ((int)llrm_os_read(handle, (unsigned char *)(buffer), count))
#define bench_write(handle, buffer, count) ((int)llrm_os_write_file(handle, (const unsigned char *)(buffer), count))
#define bench_close(handle) llrm_os_close(handle)
#else
#include <io.h>
#include <fcntl.h>
#include <sys/stat.h>
#define bench_open_read(name) open(name, O_RDONLY | O_BINARY)
#define bench_create(name) open(name, O_WRONLY | O_CREAT | O_TRUNC | O_BINARY, S_IWRITE)
#define bench_read(handle, buffer, count) read(handle, buffer, count)
#define bench_write(handle, buffer, count) write(handle, buffer, count)
#define bench_close(handle) close(handle)
#endif

#endif
