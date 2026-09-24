' Sorts and sums scores and spells a name through SORTLIB.OBJ, a library
' written in the modern language (sortlib.mod), which calls Mean# back.
DEFINT A-Z
'$INCLUDE: 'SORTLIB.BI'

DIM scores(1 TO 6) AS INTEGER
FOR i = 1 TO 6
    READ scores(i)
NEXT
DATA 42, 97, 15, 73, 88, 60

count = 6
SortScores scores(), count
FOR i = 1 TO 6
    PRINT scores(i);
NEXT
PRINT
PRINT Average#(scores(), count)

DIM grid(1 TO 2, 1 TO 3) AS INTEGER
FOR r = 1 TO 2
    FOR c = 1 TO 3
        grid(r, c) = r * 10 + c
    NEXT
NEXT
PRINT RowTotal%(grid(), 0); RowTotal%(grid(), 1)

player$ = "ada lovelace"
Upper player$
PRINT player$; " "; Initials$(player$)
END

' sortlib's Average calls this.
FUNCTION Mean# (total AS INTEGER, count AS INTEGER)
    Mean# = CDBL(total) / count
END FUNCTION
