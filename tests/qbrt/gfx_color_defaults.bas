' The colour a graphics statement uses when it is given none: after COLOR, after LINE with a colour, in
' PSET, PRESET, LINE, CIRCLE, PAINT.
SCREEN 9
COLOR 12, 3
PSET (20, 20)
PRESET (24, 20)
LINE (30, 20)-(60, 40)
CIRCLE (100, 40), 20
PAINT (100, 40), 5, 12
COLOR 10
PSET (20, 60)
PRESET (24, 60)
LINE (30, 60)-(60, 80)
CIRCLE (160, 70), 20
LINE (200, 20)-(260, 60), 6, BF
LINE (290, 20)-(300, 40)
COLOR 9, 0
PSET (20, 100)
PRESET (24, 100)
LINE (30, 100)-(60, 120)
SLEEP
