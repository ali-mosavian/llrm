' PAINT with the paint colour or the border left out: both are -1, the foreground and the paint colour.
DEFINT A-Z
OPEN "L.TXT" FOR OUTPUT AS #1
SCREEN 1
COLOR 1, 0
LINE (20, 20)-(60, 60), 2, B
PAINT (30, 30)
PRINT #1, POINT(30, 30); POINT(10, 10); POINT(20, 20)
LINE (100, 20)-(140, 60), 3, B
PAINT (110, 30), , 3
PRINT #1, POINT(110, 30); POINT(90, 10); POINT(100, 20)
LINE (200, 20)-(240, 60), 1, B
PAINT STEP(-10, 0), 2
PRINT #1, POINT(230, 30)
CLOSE #1
