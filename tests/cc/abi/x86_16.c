/* The m16 ABI clang is told: compile-only. A wrong size here once passed 59 QCport modules whose far pointers were two bytes. */
#define far __attribute__((address_space(1)))
#define huge __attribute__((address_space(3)))
typedef struct { void (*f)(int); int x; char c; } Record;
_Static_assert(sizeof(int) == 2 && sizeof(long) == 4 && sizeof(long long) == 8, "integers");
_Static_assert(sizeof(int *) == 2 && sizeof(int far *) == 4 && sizeof(int huge *) == 4, "data pointers");
_Static_assert(sizeof(void (*)(void)) == 4, "function pointers are far in the medium model");
_Static_assert(sizeof(__SIZE_TYPE__) == 2 && sizeof(__PTRDIFF_TYPE__) == 2, "size_t, ptrdiff_t");
_Static_assert(sizeof(Record) == 7 && __builtin_offsetof(Record, c) == 6, "byte-packed records");
_Static_assert(sizeof(long double) == 10, "x87 extended");
int unused;
