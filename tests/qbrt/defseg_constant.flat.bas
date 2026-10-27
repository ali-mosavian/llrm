' defseg_constant.bas at one address space: the same reads, then video memory at its linear address under DEF SEG = 0.
DEF SEG = 0
PRINT PEEK(1047) - PEEK(&H417)
DEF SEG = &HB800
POKE 3998, 65
x% = PEEK(3998)
PRINT x%
DEF SEG = 0
PRINT PEEK(&HB8000 + 3998) - x%
DEF SEG
DEF SEG
LOCATE 25, 1
