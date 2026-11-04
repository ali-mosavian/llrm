' RUN: llrm-qb %s -fqb-runtime=llrm --isel={dag | old} -Os -fno-inline-functions -fno-inline-functions-called-once -march=i486 -m32 -S -o /dev/stdout
' LiveRangeShrink put `sum + Tmul(...)` above the call in the loop: the first sum read an unset register, the last result was dropped (#5768).
' CHECK-LABEL: ROW proc
' CHECK: call TMUL
' CHECK: add {{.*}}, eax
' CHECK: jne
DEFINT A-Z
DECLARE FUNCTION Tmul& (a AS LONG, b AS LONG)
DECLARE FUNCTION Row& (v AS INTEGER)
DIM SHARED cosine(0 TO 7, 0 TO 7) AS LONG
DIM SHARED coef(0 TO 63) AS LONG
DIM i AS INTEGER, k AS INTEGER
FOR i = 0 TO 7
    FOR k = 0 TO 7
        cosine(i, k) = ((i * 37 + k * 91) MOD 1005) - 502
    NEXT
NEXT
FOR i = 0 TO 63
    coef(i) = ((i * 53) MOD 301) - 150
NEXT
PRINT LTRIM$(STR$(Row&(3)))

FUNCTION Tmul& (a AS LONG, b AS LONG)
    Tmul& = (a * b) \ 1024
END FUNCTION

FUNCTION Row& (v AS INTEGER)
    DIM sum AS LONG, u AS INTEGER
    sum = 0
    FOR u = 0 TO 7
        sum = sum + Tmul&(cosine(2, u), coef(v * 8 + u) * 1024)
    NEXT
    Row& = sum
END FUNCTION
