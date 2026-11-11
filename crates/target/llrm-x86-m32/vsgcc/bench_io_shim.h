/* The file API of bench/bench_io.h as plain external calls: llrm_os.h is Watcom-only (#pragma aux) and gcc and clang predefine
   __INT_MAX__, so the real header cannot serve three compilers. wrap.py pastes this in; stub.s holds the entry points and
   harness.py emulates them, so every compiler's column makes the same calls and none is priced for the work behind one. */
extern int vs_open_read(const char *name);
extern int vs_create(const char *name);
extern int vs_read(int handle, void *buffer, int count);
extern int vs_write(int handle, const void *buffer, int count);
extern int vs_close(int handle);
#define bench_open_read(name) vs_open_read(name)
#define bench_create(name) vs_create(name)
#define bench_read(handle, buffer, count) vs_read(handle, buffer, count)
#define bench_write(handle, buffer, count) vs_write(handle, buffer, count)
#define bench_close(handle) vs_close(handle)
