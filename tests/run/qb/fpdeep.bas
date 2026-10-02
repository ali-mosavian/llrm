' Two floating-point values live on the x87 stack at once here, and both of
' them came from the same address.
'
' Every other float program in this suite is one operation deep -- a + b, a * b
' -- so each expression loads two different addresses and pops them both. That
' left a whole shape uncovered. `p(i) * p(i)` compiles to two consecutive
' `fld dword ptr [si]`, and a load-forwarding pass that reads si as the
' destination register sees a load and a redundant reload of the same bytes.
' It is neither: an fld pushes. Deleting the second leaves one value where the
' program wants two, and every x87 slot after it is off by one. The pass did
' exactly that, and bench/fpbench.bas printed -2147483648 for every coordinate.
'
' So the point of this program is the two-deep expression through a variable
' index, which is the only way to get a register-addressed operand. Results
' are exact in SINGLE and integral after CLNG: what is under test is the shape,
' not QuickBASIC's rounding.
DEFINT A-Z
DIM p(1 TO 3) AS SINGLE
DIM q AS SINGLE, k AS SINGLE
DIM d AS DOUBLE, e AS DOUBLE
DIM i AS INTEGER

p(1) = 12: p(2) = 28: p(3) = 60
k = 4

FOR i = 1 TO 3
    q = p(i) * p(i)
    PRINT "SQ"; i; "="; CLNG(q)
    q = (p(i) * p(i)) / (p(i) + p(i))
    PRINT "RATIO"; i; "="; CLNG(q)
    q = (p(i) - k) / (p(i) + k)
    PRINT "MIX"; i; "="; CLNG(q * 1024)
NEXT i

d = 12
e = (d * d) / (d + d)
PRINT "DSQ="; CLNG(d * d)
PRINT "DRATIO="; CLNG(e)

PRINT "DONE"
END
