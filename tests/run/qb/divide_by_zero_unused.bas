' flags: -O0 | -O1 | -O2 | -Os
' mask: Division by zero.*
' A zero divisor ends the program with BC's error even where the quotient is never used (-O2 deleted the undefined sdiv and printed on).
PRINT "start"
y% = 0
x% = 5 \ y%
PRINT "not reached"
