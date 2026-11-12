/* Files by name for the benches: llrm's OS layer under llrm-c, which predefines GCC's macros (__INT_MAX__) as the
   reference compilers do not, and their own <io.h> calls otherwise. Binary, in the current directory. A handle
   is below zero on an error; bench_read is 0 at the end of the file.

   A twin of a Nib program that does file I/O (bench/NAME/twin) defines BENCH_IO_FUNCTIONS first: the five calls are then functions of the
   program's object, as std.io's are in the Nib program's, so every compiler pays for them in its own code. Without it they are macros
   and the object holds only the calls. */
#ifndef BENCH_IO_H
#define BENCH_IO_H

#ifdef __INT_MAX__
#include "llrm_os.h"
#define BENCH_IO_OPEN_READ(name) ((int)llrm_os_open(name, 0))
#define BENCH_IO_CREATE(name) ((int)llrm_os_create(name))
#define BENCH_IO_READ(handle, buffer, count) ((int)llrm_os_read(handle, (unsigned char *)(buffer), count))
#define BENCH_IO_WRITE(handle, buffer, count) ((int)llrm_os_write_file(handle, (const unsigned char *)(buffer), count))
#define BENCH_IO_CLOSE(handle) ((int)llrm_os_close(handle))
#else
#include <io.h>
#include <fcntl.h>
#define BENCH_IO_OPEN_READ(name) open(name, O_RDONLY | O_BINARY)
#define BENCH_IO_CREATE(name) open(name, O_WRONLY | O_CREAT | O_TRUNC | O_BINARY, 0x80) /* S_IWRITE: <sys/stat.h> is not found as a DOS compiler spells it */
#define BENCH_IO_READ(handle, buffer, count) read(handle, buffer, count)
#define BENCH_IO_WRITE(handle, buffer, count) write(handle, buffer, count)
#define BENCH_IO_CLOSE(handle) close(handle)
#endif

#ifdef BENCH_IO_FUNCTIONS
int bench_open_read(const char *name) { return BENCH_IO_OPEN_READ(name); }
int bench_create(const char *name) { return BENCH_IO_CREATE(name); }
int bench_read(int handle, void *buffer, int count) { return BENCH_IO_READ(handle, buffer, count); }
int bench_write(int handle, const void *buffer, int count) { return BENCH_IO_WRITE(handle, buffer, count); }
int bench_close(int handle) { return BENCH_IO_CLOSE(handle); }
#else
#define bench_open_read(name) BENCH_IO_OPEN_READ(name)
#define bench_create(name) BENCH_IO_CREATE(name)
#define bench_read(handle, buffer, count) BENCH_IO_READ(handle, buffer, count)
#define bench_write(handle, buffer, count) BENCH_IO_WRITE(handle, buffer, count)
#define bench_close(handle) ((void)BENCH_IO_CLOSE(handle))
#endif

#endif
