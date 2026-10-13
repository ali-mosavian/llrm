' textfill.bas at one address space: video memory is an address held in a variable (DEF SEG is ignored there).
DEFINT A-Z
DECLARE SUB Fill (ch, at)
DECLARE FUNCTION Checksum& ()
DECLARE FUNCTION BenchTextfill& ()
PRINT LTRIM$(STR$(BenchTextfill&))
END

FUNCTION BenchTextfill&
    Fill 65, 31
    BenchTextfill& = Checksum&
END FUNCTION

SUB Fill (ch, at)
    video& = &HB8000
    FOR o = 0 TO 3998 STEP 2
        POKE video& + o, ch + (o AND 15)
        POKE video& + o + 1, at
    NEXT
END SUB

FUNCTION Checksum&
    video& = &HB8000
    s& = 0
    FOR o = 0 TO 3999
        s& = s& + PEEK(video& + o)
    NEXT
    Checksum& = s&
END FUNCTION
