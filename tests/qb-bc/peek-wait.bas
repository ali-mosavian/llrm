' PEEK reads memory every time, whatever DEF SEG says: the BIOS tick wait ends.
DEFINT A-Z
DEF SEG = &H40
t = PEEK(&H6C)
DO
LOOP UNTIL PEEK(&H6C) <> t
POKE &H6C, 0
POKE &H6C, 1
PRINT "ticked"
