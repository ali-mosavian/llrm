// flags: -O2 -march=i486
/* A struct copy is a flat string operation: ESI, EDI and ECX, `rep movsd` then the tail, no segment operand. */
struct S { int words[14]; short tail; char last; };

void copy(struct S *to, struct S *from)
{
    *to = *from;
}
