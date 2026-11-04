/* The m32 row keeps natural struct layout (cb00b095d7): the QB runtime's structs match its assembly. */
typedef struct { void (*f)(int); int x; char c; } Record;
_Static_assert(sizeof(Record) == 12 && __builtin_offsetof(Record, c) == 8, "natural records");
int unused;
