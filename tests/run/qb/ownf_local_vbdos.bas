' dialect: vbdos
' Own frames: ON LOCAL ERROR in a caller, the error raised in a procedure below it that
' has no handler. RESUME NEXT continues in the caller after the call.
DECLARE SUB top ()
DECLARE SUB mid (d AS INTEGER)
DECLARE SUB low (d AS INTEGER)
DEFINT A-Z
top
PRINT "done"
END
SUB top
DIM n AS INTEGER
ON LOCAL ERROR GOTO th
n = 0
mid n
PRINT "top"; n
EXIT SUB
th:
PRINT "handler"; ERR
n = 1
RESUME NEXT
END SUB
SUB mid (d AS INTEGER)
DIM k AS INTEGER
k = 7
low d
PRINT "mid"; k
END SUB
SUB low (d AS INTEGER)
DIM c AS INTEGER
c = 10 \ d
PRINT "low"; c
END SUB
