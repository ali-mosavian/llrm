/* LP64: the layout of x86_64-linux, arm64-linux and arm64-glibc. */
typedef struct { void (*f)(int); int x; char c; } Record;
_Static_assert(sizeof(int) == 4 && sizeof(long) == 8 && sizeof(long long) == 8, "integers");
_Static_assert(sizeof(void *) == 8 && sizeof(void (*)(void)) == 8, "pointers");
_Static_assert(sizeof(__SIZE_TYPE__) == 8 && sizeof(__PTRDIFF_TYPE__) == 8, "size_t, ptrdiff_t");
_Static_assert(sizeof(Record) == 16 && __builtin_offsetof(Record, c) == 12, "records");
int unused;
