' CLS in a graphics mode clears the graphics screen whatever standard output is: here the test's output is a file, and
' the MS runtime still cleared the pixels (TSC's deedlines/qbdemo cuts: a 160x100 box of the fractal survived the CLS).
FOR m = 1 TO 13
  IF m = 1 OR m = 7 OR m = 9 OR m = 13 THEN
    SCREEN m
    PSET (10, 10), 1
    PSET (200, 100), 1
    CLS
    PRINT m; POINT(10, 10); POINT(200, 100)
  END IF
NEXT
SCREEN 0
