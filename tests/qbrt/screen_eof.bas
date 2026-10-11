' INKEY$ and a line of INPUT once the typed input has run out.
CLS
k$ = INKEY$
PRINT ASC(k$)
FOR i% = 1 TO 8
    k$ = INKEY$
    PRINT LEN(k$);
    IF LEN(k$) THEN PRINT ASC(k$);
NEXT
PRINT
LOCATE 25, 1
