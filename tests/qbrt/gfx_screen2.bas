' SCREEN 2 drawing: boxes in every colour, lines, circles and an arc, PAINT, text, GET and PUT.
SCREEN 2
w% = 640: h% = 200: n% = 2
FOR c% = 0 TO n% - 1
  LINE (4 + c% * (w% - 8) \ n%, 4)-(w% \ n% - 2 + c% * (w% - 8) \ n%, h% \ 6), c%, BF
  LINE (4 + c% * (w% - 8) \ n%, h% \ 5)-(w% \ n% - 2 + c% * (w% - 8) \ n%, h% \ 3), c%, B
  PSET (10 + c% * (w% - 8) \ n%, h% \ 3 + 6), c%
NEXT
LINE (0, h% \ 3 + 10)-(w% - 1, h% - 1), n% - 1
LINE (0, h% - 1)-(w% - 1, h% \ 3 + 10), n% - 2
LINE (-20, -20)-(30, 30), 1
CIRCLE (w% \ 4, h% * 2 \ 3), h% \ 6, 2
CIRCLE (w% \ 2, h% * 2 \ 3), h% \ 8, 1, , , .5
CIRCLE (w% * 3 \ 4, h% * 2 \ 3), h% \ 7, n% - 1, -.5, -4.2
PAINT (w% \ 4, h% * 2 \ 3), 1, 2
DIM a%(400)
GET (4, 4)-(40, 24), a%
PUT (w% - 60, h% - 40), a%, PSET
PUT (w% - 100, h% - 40), a%, XOR
PUT (w% - 140, h% - 40), a%, OR
PUT (w% - 180, h% - 40), a%, AND
PUT (w% - 220, h% - 40), a%, PRESET
LOCATE 3, 4: PRINT "Mode 2 text"
PRINT POINT(1, 1); POINT(w% - 1, h% - 1); POINT(-5, 2)
SLEEP
