' DEF SEG with and without a segment, PEEK and POKE through it.
x% = 258
DEF SEG
PRINT PEEK(VARPTR(x%)); PEEK(VARPTR(x%) + 1)
video% = &HB800
DEF SEG = video%
POKE 3998, 65
PRINT PEEK(3998)
DEF SEG
POKE VARPTR(x%), 7
PRINT x%
