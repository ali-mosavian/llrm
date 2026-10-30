' flags: -O1 | -O2 | -O3 | -Os
' ON ERROR with the range errors (#3506): EXP overflow is error 6, LOG of zero or less error 5; ERR, ERL and RESUME NEXT in a loop.
ON ERROR GOTO h
FOR i% = 1 TO 3
    x# = EXP(30000# * i%)
    PRINT "exp"; i%
NEXT
FOR i% = 0 TO 2
    y# = LOG(i% - 1)
    PRINT "log"; i%
NEXT
s! = 1E10
PRINT "sin"; SIN(s! * 1000)
PRINT "done"
END
h:
PRINT "err"; ERR; ERL
RESUME NEXT
