' CIRCLE (circles, ellipses by aspect, arcs, spokes), PAINT and GET/PUT in SCREEN 9.
SCREEN 9
CIRCLE (100, 80), 50, 15
CIRCLE (260, 80), 40, 14, , , 2
CIRCLE (400, 80), 30, 13, , , .5
CIRCLE (540, 80), 10, 12
CIRCLE (100, 220), 50, 11, 0, 3.14159
CIRCLE (260, 220), 40, 10, 3.14159, 0
CIRCLE (400, 220), 30, 9, -1, -2.5
CIRCLE (540, 220), 20, 8, 1, 5
PSET (3, 3), 1
CIRCLE (30, 330), 25, 7
PAINT (30, 330), 6, 7
PAINT (100, 80), 5, 15
PAINT (260, 80), 4, 14
LINE (330, 300)-(400, 340), 3, B
PAINT (350, 320), 2, 3
DIM sprite%(300)
GET (20, 305)-(50, 335), sprite%
PUT (150, 300), sprite%, PSET
PUT (200, 300), sprite%, XOR
PUT (200, 300), sprite%, XOR
PUT (250, 300), sprite%, PRESET
PUT (500, 300), sprite%, OR
PUT (540, 300), sprite%, AND
SLEEP
