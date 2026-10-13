' defseg.bas at one address space: DEF SEG is ignored, an absolute address (video memory) is held in a variable.
x% = 258
PRINT PEEK(VARPTR(x%)); PEEK(VARPTR(x%) + 1)
video& = &HB8000
POKE video& + 3998, 65
PRINT PEEK(video& + 3998)
POKE VARPTR(x%), 7
PRINT x%
