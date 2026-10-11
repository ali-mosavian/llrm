' SCREEN 0 after SCREEN 9: the text screen is clean and PRINT shows, in colour; CLS clears the key line too.
SCREEN 9
LINE (0, 0)-(100, 100), 4, BF
SCREEN 0
WIDTH 80, 25
DEF SEG = &HB800
PRINT PEEK(0); PEEK(1); PEEK(3998); PEEK(3999)
COLOR 15, 0
CLS
PRINT "hello"
PRINT PEEK(3999); PEEK(161)
LOCATE 5, 5: COLOR 12: PRINT "red"
SLEEP
