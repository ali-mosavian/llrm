' SCREEN 9 drawing: points, lines, boxes and filled boxes in the 16 colours.
SCREEN 9
FOR c% = 0 TO 15
  LINE (10 + c% * 30, 10)-(30 + c% * 30, 40), c%, BF
  LINE (10 + c% * 30, 50)-(30 + c% * 30, 80), c%, B
  PSET (20 + c% * 30, 100), c%
NEXT
LINE (0, 120)-(639, 349), 15
LINE (0, 349)-(639, 120), 14
LINE (100, 130)-(200, 130), 12
LINE (100, 140)-(100, 200), 11
LINE (300, 200)-(400, 150), 10
LINE (-20, -20)-(30, 30), 9
LINE (600, 300)-(700, 400), 8, B
PSET (320, 175)
PRESET (321, 175)
LINE (500, 140)-(530, 200), 13
LINE (560, 200)-(540, 140), 7
LINE (450, 320)-(380, 330), 6
LINE (380, 340)-(450, 345), 5
LINE (520, 260)-(520, 330), 4
LINE (600, 260)-(550, 262), 3
a! = 10.5: b! = 300.2
PSET (a!, b!), 5
LINE (100.4, 330.6)-(120.2, 340.7), 4
LINE (a! * 4, b!)-(a! * 5, b! + 9), 3, BF
PALETTE 4, 63
PALETTE 5, 18
LOCATE 24, 40: PRINT POINT(a!, b!); POINT(1, 1); POINT(-5, 2)
COLOR 12, 1
LOCATE 20, 10: PRINT "Graphics text"
SLEEP
