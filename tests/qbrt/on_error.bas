' ON ERROR GOTO with RESUME NEXT: the error number in the handler, the
' statement after the failing one run next, a raised error, and ON ERROR GOTO 0.
ON ERROR GOTO handler
OPEN "NOSUCH.FIL" FOR INPUT AS #1
PRINT "after open"
ERROR 11
PRINT "after error 11"
x% = 7
y% = x% \ (x% - 7)
PRINT "after division"
ON ERROR GOTO 0
PRINT "handler off"
END

handler:
PRINT "error"; ERR
RESUME NEXT
