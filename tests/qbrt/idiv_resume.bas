' ON ERROR GOTO with RESUME NEXT after the divide fault of -32768 \ -1 and -32768 MOD -1: BC's handler sees error 11 (ERL 0) and the program
' goes on past each divide (oracle: qbmeta/oracle/H.BAS).
DEFINT A-Z
ON ERROR GOTO h
READ a, b
DATA -32768, -1
PRINT "idiv:"; a \ b
PRINT "mod:"; a MOD b
PRINT "done"
END
h:
PRINT "ERR="; ERR; "ERL="; ERL
RESUME NEXT
