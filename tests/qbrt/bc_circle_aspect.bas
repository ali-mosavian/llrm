' CIRCLE's aspect ratio of zero or below: BC maps it (rt/circle.asm B$CASP) as the interpreters did
' and draws; the llrm runtime raised Illegal function call, which ended GORILLA when a banana hit.
' Each line is the aspect, the error, and the lit pixels in the circle's box.
DEFINT A-Z
DIM a!(1 TO 9)
FOR i = 1 TO 9: READ a!(i): NEXT
DATA -1.57,-1,-.5,-3,0,.5,1,1.57,2
SCREEN 9
OPEN "L.TXT" FOR OUTPUT AS #1
ON ERROR GOTO oops
FOR i = 1 TO 9
  CLS
  e = 0
  CIRCLE (100, 100), 20, 4, , , a!(i)
  n = 0
  FOR y = 60 TO 140
    FOR x = 40 TO 160
      IF POINT(x, y) <> 0 THEN n = n + 1
    NEXT
  NEXT
  PRINT #1, a!(i); "err"; e; "pixels"; n
NEXT
CLOSE #1
END
oops:
e = ERR
RESUME NEXT
