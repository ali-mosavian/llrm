' A LONG array, deliberately: an array element's own address is
' register-indexed (`[si+addr]`), which the corpus has zero of and qbopt's
' call-site absorption cannot see through at all. And a chained subexpression
' (`s \ 1000 + 1`) that feeds straight into a second divide without ever
' being named, the same shape as suite/nbody.bas's `falloff`.
DEFINT A-Z
CONST N = 4
DIM x(N) AS LONG
DIM y(N) AS LONG
DIM p AS LONG, s AS LONG, f AS LONG, r AS LONG
DIM i AS INTEGER
DIM tag AS STRING

FOR i = 0 TO N - 1
    x(i) = (i + 1) * 100000
    y(i) = (i + 2) * 100000
NEXT

FOR i = 0 TO N - 1
    p = x(i) * y(i)
    s = p \ 1000 + 1000000
    f = 50000 \ (s \ 1000 + 1)
    r = (x(i) * f) \ 512
    tag = LTRIM$(STR$(i))
    PRINT "P" + tag + "="; p
    PRINT "S" + tag + "="; s
    PRINT "F" + tag + "="; f
    PRINT "R" + tag + "="; r
NEXT

PRINT "DONE"
END
