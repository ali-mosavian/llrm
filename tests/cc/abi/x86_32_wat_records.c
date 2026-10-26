/* Watcom packs structs by default. */
typedef struct { void (*f)(int); int x; char c; } Record;
_Static_assert(sizeof(Record) == 9 && __builtin_offsetof(Record, c) == 8, "byte-packed records");
int unused;
