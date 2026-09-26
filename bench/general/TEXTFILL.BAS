DEFINT A-Z
DECLARE SUB Fill (ch, at)
DECLARE FUNCTION Checksum& ()
Fill 65, 31
s& = Checksum&
OPEN "TEXTFILL.OUT" FOR OUTPUT AS #1
PRINT #1, "RESULT="; s&
CLOSE #1
END

SUB Fill (ch, at)
    DEF SEG = &HB800
    FOR o = 0 TO 3998 STEP 2
        POKE o, ch + (o AND 15)
        POKE o + 1, at
    NEXT
    DEF SEG
END SUB

FUNCTION Checksum&
    DEF SEG = &HB800
    s& = 0
    FOR o = 0 TO 3999
        s& = s& + PEEK(o)
    NEXT
    DEF SEG
    Checksum& = s&
END FUNCTION
