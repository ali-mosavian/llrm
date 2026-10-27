' GORILLA.BAS: its dynamic arrays are DIMmed before InitVars sets ON ERROR.
'$DYNAMIC
DIM SHARED a(x)
ON ERROR GOTO h
PRINT a(0)
END
h: RESUME NEXT
SUB s
DIM w(1 TO 2)
w(1) = 1
END SUB
