DEFINT A-Z
DECLARE SUB Turn ()
DIM SHARED kon(100), pa
pa = 355
kon(75) = 1
FOR i = 1 TO 10
  Turn
NEXT
PRINT "PA="; pa
PRINT "DONE"
END

SUB Turn
WHILE kon(75) = 1
pa = pa + 1
IF pa > 360 THEN pa = 0 + (pa - 360)
GOTO 101
WEND
101
WHILE kon(77) = 1
pa = pa - 1
IF pa < 0 THEN pa = 360 + pa
GOTO 102
WEND
102
END SUB
