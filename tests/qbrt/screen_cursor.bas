' The cursor is hidden while the program runs (bit 5 of the cursor start line,
' 0040:0061), shown by LOCATE ,,1 and hidden by LOCATE ,,0.
CLS
DEF SEG = 0
PRINT PEEK(&H461) AND 32
LOCATE , , 1
PRINT PEEK(&H461) AND 32
LOCATE , , 0
PRINT PEEK(&H461) AND 32
LOCATE 25, 1
