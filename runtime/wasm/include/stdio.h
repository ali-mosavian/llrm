/* Only what the host video's PPM dump names; it writes nothing on wasm32. */
#ifndef LLRM_WASM_STDIO_H
#define LLRM_WASM_STDIO_H
typedef struct llrm_file FILE;
static inline FILE *fopen(const char *path, const char *mode) { (void)path; (void)mode; return 0; }
static inline int fclose(FILE *f) { (void)f; return 0; }
typedef unsigned long llrm_size;
static inline llrm_size fwrite(const void *p, llrm_size size, llrm_size count, FILE *f) { (void)p; (void)f; return size * count; }
#define fprintf(f, ...) ((void)(f), 0)
#endif
