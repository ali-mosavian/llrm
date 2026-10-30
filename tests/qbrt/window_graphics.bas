' WINDOW: statements take logical coordinates (y up, or down for SCREEN) mapped onto the screen or the viewport, STEP works in
' logical units, CIRCLE's radius scales, POINT maps; WINDOW alone turns it off. The screen is hashed by POINT.
DEFINT A-Z
DECLARE SUB Dump (tag$)
SCREEN 13
WINDOW (0, 0)-(10, 10)
PSET (5, 5), 14
LINE (0, 0)-(10, 10), 2
LINE (1, 9)-(4, 6), 4, B
CIRCLE (7.5, 2.5), 2, 3
PSET STEP(1.5, 1), 9
LINE -STEP(-2, -1), 12
x! = 3.25: y! = 7.75
PSET (x!, y!), 15
Dump "window"
WINDOW SCREEN (-50, -20)-(50, 20)
LINE (-50, -20)-(50, 20), 6
CIRCLE (0, 0), 10, 5
PSET (25.5, 10.25), 1
PRINT POINT(0, 0); POINT(25.5, 10.25)
Dump "window screen"
VIEW (60, 40)-(260, 160), , 15
WINDOW (-1, -1)-(1, 1)
CIRCLE (0, 0), .5, 14
LINE (-1, -1)-(1, 1), 10
Dump "window view"
WINDOW
LINE (0, 0)-(100, 100), 13
Dump "off"
ON ERROR GOTO h
WINDOW (1, 1)-(1, 5)
WINDOW (1, 2)-(3, 2)
SCREEN 0
WINDOW (0, 0)-(1, 1)
END
h:
PRINT "error"; ERR
RESUME NEXT

SUB Dump (tag$)
  h& = 0
  FOR y = 0 TO 199
    FOR x = 0 TO 319
      h& = (h& * 31 + POINT(x, y)) AND &HFFFFFF
    NEXT
  NEXT
  PRINT tag$; h&
END SUB
