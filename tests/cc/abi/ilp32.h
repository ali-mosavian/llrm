/* ILP32: the layout of x86_32-linux, x86_32-dos and x86_32-wat's integers and pointers. */
_Static_assert(sizeof(int) == 4 && sizeof(long) == 4 && sizeof(long long) == 8, "integers");
_Static_assert(sizeof(void *) == 4 && sizeof(void (*)(void)) == 4, "pointers");
_Static_assert(sizeof(__SIZE_TYPE__) == 4 && sizeof(__PTRDIFF_TYPE__) == 4, "size_t, ptrdiff_t");
int unused;
