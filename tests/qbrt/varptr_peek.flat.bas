' varptr_peek.bas at one address space: VARPTR is the whole address (DEF SEG is ignored there).
' PEEK and POKE through VARSEG and VARPTR of a variable. On dos32 the address is whole and the segment is 0: the
' paragraph and low nibble of the address were once what VARSEG and VARPTR gave, which broke above 1 MB.
x% = 258
PRINT PEEK(VARPTR(x%)); PEEK(VARPTR(x%) + 1)
POKE VARPTR(x%), 7
PRINT x%
y% = 3
PRINT PEEK(VARPTR(y%))
