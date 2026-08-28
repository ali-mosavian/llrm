' Every case prints one line, and the last line is DONE. Anything else --
' LOCATE, SCREEN, COLOR -- and the output cannot be redirected to a file.
'
' The values are asymmetric in every bit position, so an operand order that
' is backwards gives a different answer rather than the same one.
DEFINT A-Z
DIM a AS LONG, b AS LONG, r AS LONG
DIM lo AS LONG, hi AS LONG, one AS LONG

a = 305419896                   ' &H12345678
b = 252645135                   ' &H0F0F0F0F
one = 1

PRINT "AND="; a AND b
PRINT "OR="; a OR b
PRINT "XOR="; a XOR b
PRINT "ADD="; a + b
PRINT "SUB="; a - b
PRINT "NEG="; -a

' the chain: a value that stays in a register across several operations
r = a
r = r AND b
r = r XOR a
r = r + b
PRINT "CHAIN="; r

' carry across the halfway boundary. A pass that widens correctly and one
' that drops the carry agree on everything above and disagree here.
lo = 65535                      ' &H0000FFFF
PRINT "CARRY="; lo + one
hi = 65536                      ' &H00010000
PRINT "BORROW="; hi - one

' two integers two bytes apart have the same displacements as one long's
' halves; only the register pairing tells them apart
DIM i AS INTEGER, j AS INTEGER
i = 258
j = 772
PRINT "INTS="; i; j

PRINT "DONE"
