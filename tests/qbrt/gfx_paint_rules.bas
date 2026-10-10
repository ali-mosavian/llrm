' PAINT rules, one picture per zone: a border colour stops it, the fill colour does not, nothing happens
' from a start on the border colour, a fill with no border colour stops at its own colour.
SCREEN 9
' A: plain box
LINE (10, 10)-(110, 90), 1, B
PAINT (50, 50), 2, 1
' B: a line in the fill colour across the box does not stop the fill
LINE (130, 10)-(230, 90), 1, B
LINE (180, 12)-(180, 88), 2
PAINT (150, 50), 2, 1
' C: a pocket of another colour inside is filled too
LINE (250, 10)-(350, 90), 1, B
LINE (280, 30)-(320, 60), 3, BF
PAINT (260, 20), 2, 1
' D: no border colour: stops at the fill colour itself
LINE (370, 10)-(470, 90), 4, B
LINE (420, 12)-(420, 88), 4
PAINT (390, 50), 4
' E: a start on the border colour paints nothing
LINE (490, 10)-(590, 90), 1, B
PAINT (490, 10), 2, 1
PAINT (540, 10), 2, 1
' F: a channel of the fill colour leads to a second room
LINE (10, 120)-(110, 200), 1, B
LINE (60, 122)-(60, 198), 1
LINE (60, 150)-(60, 160), 2
PAINT (30, 160), 2, 1
' G: a wall of the border colour with a gap; the fill leaks round it
LINE (130, 120)-(230, 200), 1, B
LINE (180, 122)-(180, 170), 1
PAINT (150, 150), 5, 1
' H: fill colour equals the colour of the area (nothing changes), then a different one
LINE (250, 120)-(350, 200), 1, B
PAINT (260, 130), 0, 1
PAINT (260, 130), 6, 1
' I: nested boxes: the inner one is not reached
LINE (370, 120)-(470, 200), 1, B
LINE (390, 140)-(450, 180), 1, B
PAINT (380, 130), 7, 1
' J: the whole screen outside everything
PAINT (5, 300), 8, 1
' K: a diagonal wall (touching pixels corner to corner): does the fill cross it?
LINE (490, 120)-(590, 200), 1, B
LINE (490, 120)-(590, 200), 1
LINE (492, 122)-(590, 198), 1
PAINT (500, 190), 9, 1
' L: a shallow wall and a steep wall
LINE (10, 220)-(110, 300), 1, B
LINE (10, 230)-(110, 290), 1
PAINT (20, 295), 10, 1
LINE (130, 220)-(230, 300), 1, B
LINE (150, 222)-(210, 298), 1
PAINT (140, 295), 11, 1
SLEEP
