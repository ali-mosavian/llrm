' Fill and sum a 1024-entry LONG ring, in the kernel; Nib has no globals.
DEFINT A-Z
DECLARE FUNCTION BenchRing& ()
PRINT LTRIM$(STR$(BenchRing&))
END

FUNCTION BenchRing&
    DIM buf(1 TO 1024) AS LONG
    FOR i = 1 TO 1024: buf(i) = i * 3 - 7: NEXT
    s& = 0
    FOR i = 0 TO 5999
        s& = s& + buf(((i * 5 + 3) AND 1023) + 1)
    NEXT
    BenchRing& = s&
END FUNCTION
