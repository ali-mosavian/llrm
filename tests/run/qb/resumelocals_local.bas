' dialect: pds71
' A local written before the statement that raises and read after it, under each kind of
' RESUME of a procedure's own handler (ON LOCAL ERROR): NEXT, the statement again, and a
' label. The handler is given the locals RESUME can read, none of which may be lost.
DECLARE SUB Nxt ()
DECLARE SUB Agn ()
DECLARE SUB Lab ()
Nxt
Agn
Lab
END
SUB Nxt
    ON LOCAL ERROR GOTO nh
    a% = 7
    ERROR 5
    PRINT "next"; a%
    EXIT SUB
nh:
    PRINT "err"; ERR
    RESUME NEXT
END SUB
SUB Agn
    ON LOCAL ERROR GOTO ah
    b% = 20
    r% = 10 \ den%
    PRINT "again"; b%; r%
    EXIT SUB
ah:
    PRINT "err"; ERR
    den% = 2
    RESUME
END SUB
SUB Lab
    ON LOCAL ERROR GOTO lh
    c% = 41 + INT(RND * 0)
    ERROR 7
    EXIT SUB
done:
    PRINT "label"; c%
    EXIT SUB
lh:
    PRINT "err"; ERR
    RESUME done
END SUB
