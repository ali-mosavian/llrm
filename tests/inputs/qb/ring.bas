DEFINT A-Z
DECLARE FUNCTION RingSum& (n)
'$DYNAMIC
DIM SHARED buf(1 TO 1024) AS LONG
FOR i = 1 TO 1024: buf(i) = i * 3 - 7: NEXT
OPEN "RING.OUT" FOR OUTPUT AS #1
PRINT #1, "RESULT="; RingSum&(6000)
CLOSE #1
END

FUNCTION RingSum& (n)
    s& = 0
    FOR i = 0 TO n - 1
        s& = s& + buf(((i * 5 + 3) AND 1023) + 1)
    NEXT
    RingSum& = s&
END FUNCTION
