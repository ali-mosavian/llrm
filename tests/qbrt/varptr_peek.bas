' PEEK and POKE through VARSEG and VARPTR of a variable. On dos32 the address is whole and the segment is 0: the
' paragraph and low nibble of the address were once what VARSEG and VARPTR gave, which broke above 1 MB.
x% = 258
DEF SEG = VARSEG(x%)
PRINT PEEK(VARPTR(x%)); PEEK(VARPTR(x%) + 1)
POKE VARPTR(x%), 7
PRINT x%
DEF SEG
y% = 3
DEF SEG = VARSEG(y%)
PRINT PEEK(VARPTR(y%))
