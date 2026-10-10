' Control characters in PRINT on the screen.
CLS
PRINT "a"; CHR$(13); "b"; : PRINT "|"
PRINT "a"; CHR$(10); "b"; : PRINT "|"
PRINT "a"; CHR$(13); CHR$(10); "b"; : PRINT "|"
PRINT "tab"; CHR$(9); "x"; CHR$(9); "yy"
PRINT "back"; CHR$(8); "!"
PRINT "bell"; CHR$(7); "!"
PRINT "up"; CHR$(30); "U"
PRINT "abcdef"; CHR$(29); CHR$(29); "XY"
PRINT "abcdef"; CHR$(11); "H"
PRINT "right"; CHR$(28); "R"
LOCATE 20, 1: PRINT "ab"; CHR$(31); "D"
LOCATE 25, 1: PRINT "out"; CHR$(30); "O"
LOCATE 25, 1
