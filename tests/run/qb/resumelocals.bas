' A local written before the statement that raises and read after it, under RESUME NEXT and
' RESUME (the statement again, once the handler has removed the cause): the handler is given
' the locals RESUME can read, none of which may be lost on the way through it.
DECLARE SUB Nxt ()
DECLARE SUB Agn ()
DIM SHARED den AS INTEGER
ON ERROR GOTO h
Nxt
Agn
END
h:
PRINT "err"; ERR
IF ERR = 5 THEN RESUME NEXT
den = 2
RESUME
SUB Nxt
    a% = 7
    ERROR 5
    PRINT "next"; a%
END SUB
SUB Agn
    b% = 20
    r% = 10 \ den
    PRINT "again"; b%; r%
END SUB
