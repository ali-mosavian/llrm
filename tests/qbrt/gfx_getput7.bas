' GET and PUT in SCREEN 7: rectangles at every alignment, and each way of putting them, whole.
SCREEN 7
DEFINT A-Z
DIM s1(700), s2(700), s3(700), s4(700)
FOR i = 0 TO 30
    LINE (i * 7 MOD 300, i * 3 MOD 90)-(i * 11 MOD 300 + 20, i * 5 MOD 100 + 10), 1 + i MOD 15
    CIRCLE (i * 13 MOD 300 + 10, i * 9 MOD 80 + 20), 6 + i MOD 9, 1 + (i * 3) MOD 15
    PSET (i * 17 MOD 300, i * 11 MOD 90 + 5), 1 + i MOD 15
NEXT
LINE (4, 4)-(40, 24), 15, B
GET (3, 5)-(18, 20), s1
GET (8, 8)-(20, 15), s2
GET (40, 30)-(40, 60), s3
GET (16, 16)-(31, 23), s4
PUT (100, 10), s1, PSET
PUT (125, 10), s1, PRESET
PUT (101, 40), s2, PSET
PUT (122, 40), s2, PRESET
PUT (143, 41), s3, PSET
PUT (101, 70), s1, OR
PUT (126, 70), s1, AND
PUT (151, 71), s1, XOR
PUT (176, 71), s2, XOR
PUT (176, 71), s2, XOR
PUT (200, 72), s4, PSET
PUT (216, 72), s4, XOR
PUT (232, 73), s4, OR
PUT (3, 3), s1, PSET
SLEEP
