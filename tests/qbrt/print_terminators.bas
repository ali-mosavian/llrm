' Every PRINT terminator on every value type: comma zones, semicolons, end of statement.
a% = 7
b& = 100000
c$ = "ab"
PRINT a%, a%
PRINT a%; a%
PRINT b&, b&
PRINT b&; b&
PRINT c$, c$
PRINT c$; c$
PRINT a%, b&, c$
PRINT -a%; -b&; c$
