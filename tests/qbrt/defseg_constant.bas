' DEF SEG = a constant: PEEK(1047) is the keyboard flags byte, and a POKE under &HB800 reads back.
DEF SEG = 0
PRINT PEEK(1047) - PEEK(&H417)
DEF SEG = &HB800
POKE 3998, 65
PRINT PEEK(3998)
PRINT 0
DEF SEG
LOCATE 25, 1
