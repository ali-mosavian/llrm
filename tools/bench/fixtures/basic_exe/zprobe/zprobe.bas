DEFINT A-Z
DECLARE FUNCTION BenchProbe& ()
PRINT LTRIM$(STR$(BenchProbe&))
END

FUNCTION BenchProbe&
    DIM v$(1 TO 19)
    DIM i AS INTEGER
    FOR i = 1 TO 19
        READ v$(i)
        PRINT i; "["; v$(i); "]"
    NEXT i
    BenchProbe& = 19
END FUNCTION

DATA 1, 2 ,3
DATA   "a b" ,  "c d" , d e ,  f g
DATA 1,,2
DATA   x   
DATA "q  r"  ,  s  ' note
DATA 	7,	8
DATA -1, +2,  3.5 , 1E3
