' GORILLA's sun with the "o" mouth (a CIRCLE painted with colour 0 to border 0) in SCREEN 9: on dos32 the
' paint ran into string space and the game ended with String space corrupt.
SCREEN 9
pi# = 3.141592653589793#
x = 320: y = 25
LINE (x - 22, y - 18)-(x + 22, y + 18), 0, BF
CIRCLE (x, y), 12, 3
PAINT (x, y), 3
LINE (x - 20, y)-(x + 20, y), 3
CIRCLE (x, y + 5), 2.9, 0
PAINT (x, y + 5), 0, 0
CIRCLE (x - 3, y - 2), 1, 0
PSET (x - 3, y - 2), 0
a$ = "after"
FOR i = 1 TO 50: b$ = b$ + "x": NEXT
PRINT a$; LEN(b$); POINT(x, y + 5); POINT(x, y)
