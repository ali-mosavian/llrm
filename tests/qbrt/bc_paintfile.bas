' PAINT with a file open: the open file took the string space for the heap,
' and the fill's queue is made of string space.
DEFINT A-Z
OPEN "L.TXT" FOR OUTPUT AS #1
SCREEN 13
LINE (260, 150)-(280, 170), 3, B
PAINT (270, 160), 14, 3
PRINT #1, "painted"; POINT(270, 160); POINT(250, 150); POINT(260, 150)
CLOSE #1
