' ERL: the last numbered line at or before the faulting statement, 0 before
' any, for errors ERROR raises and errors runtime calls raise.
DEFINT A-Z
DIM s AS STRING, k AS INTEGER
ON ERROR GOTO handler
ERROR 11
100 ERROR 12
k = -1
s = SPACE$(k)
200 k = 300: s = CHR$(k)
PRINT "DONE"
END
handler:
PRINT "ERL="; ERR; ERL
RESUME NEXT
