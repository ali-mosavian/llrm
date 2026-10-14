' DIM of a scalar already used: what BC does where the DIM stands. Each line is the variable
' by its suffixed name and by the declared one.
OPEN "L.TXT" FOR OUTPUT AS #1
v& = 5
DIM v AS LONG
PRINT #1, v&; v
n% = 7
DIM SHARED n AS INTEGER
PRINT #1, n%; n
d# = 1.25
DIM d AS DOUBLE
PRINT #1, d#; d
s$ = "abc"
DIM s AS STRING
PRINT #1, "["; s$; "]["; s; "]"
FOR i% = 1 TO 3
  DIM w AS INTEGER
  PRINT #1, w;
  w = w + 9
NEXT
PRINT #1,
CLOSE #1
