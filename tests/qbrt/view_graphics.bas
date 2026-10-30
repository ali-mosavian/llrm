' VIEW: drawing is cut to the viewport, an ordinary VIEW moves the origin to its corner (VIEW SCREEN does not), the fill and
' border are drawn, the cursor goes to the centre; the screen is hashed by POINT.
DEFINT A-Z
DECLARE SUB Dump (tag$)
SCREEN 13
VIEW (20, 10)-(120, 90), 4, 15
PSET (0, 0), 14
LINE (-5, 5)-(200, 40), 2
CIRCLE (50, 40), 30, 3
PAINT (50, 40), 5, 3
PSET STEP(0, 0), 7
Dump "view"
DIM a(400) AS INTEGER
GET (5, 5)-(30, 25), a
PUT (60, 50), a, XOR
DRAW "BM10,60 R30 D10 L30"
PSET (3, 3), 9
PRINT POINT(3, 3); POINT(0, 0)
CLS
Dump "view ops"
VIEW SCREEN (150, 100)-(250, 180), 1, 9
PSET (160, 110), 14
LINE (140, 90)-(300, 190), 12, B
Dump "view screen"
VIEW
LINE (0, 0)-(319, 199), 6
Dump "reset"
ON ERROR GOTO h
VIEW (10, 10)-(10, 50)
VIEW (0, 0)-(400, 50)
SCREEN 0
VIEW (0, 0)-(10, 10)
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
