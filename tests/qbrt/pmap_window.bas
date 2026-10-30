' PMAP maps a logical point to a pixel of the viewport and back, with and without WINDOW.
DEFINT A-Z
OPEN "L.TXT" FOR OUTPUT AS #1
SCREEN 7
FOR f = 0 TO 3
  PRINT #1, PMAP(37.4, f); PMAP(-2, f); PMAP(199.5, f)
NEXT
VIEW (20, 10)-(219, 109)
WINDOW (-2.5, -1)-(2.5, 1)
FOR f = 0 TO 3
  PRINT #1, PMAP(0.7, f); PMAP(-1.3, f); PMAP(33, f); PMAP(99.5, f)
NEXT
WINDOW SCREEN (0, 0)-(10, 8)
FOR f = 0 TO 3
  PRINT #1, PMAP(3.3, f); PMAP(7, f); PMAP(150, f)
NEXT
WINDOW
VIEW
CLOSE #1
