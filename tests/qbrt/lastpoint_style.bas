' LINE with a 16-bit style, alone and with B and BF
DEFINT A-Z
DECLARE SUB Mark ()
DIM SHARED hue
DIM arr(200)
OPEN "L.TXT" FOR OUTPUT AS #1
SCREEN 13
Mark
LINE (0, 0)-(31, 0), 4, , &HF0F0: LINE (0, 2)-(31, 2), 4, , &H8888
LINE (0, 4)-(31, 4), 4, , &H1234
LINE (0, 6)-(9, 9), 4, B, &HAAAA
LINE (40, 6)-(49, 9), 4, BF, &H5555
FOR y = 0 TO 199
    sum& = 0
    FOR x = 0 TO 319: sum& = (sum& * 31 + POINT(x, y) + 1) AND 16777215: NEXT
    PRINT #1, y; sum&
NEXT
CLOSE #1
END
SUB Mark
    PSET STEP(0, 0), hue
    hue = hue MOD 15 + 1
END SUB
