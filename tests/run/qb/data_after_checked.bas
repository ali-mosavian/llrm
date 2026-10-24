' flags: -O1 | -O2 | -O3 | -Os
' dialect: qb45
' DATA straight after a statement with a range check: READ enters at DATA, and the statement's EXP operand crossed into that entry (invalid HIR).
DIM a AS DOUBLE, b AS DOUBLE
READ a, b
PRINT EXP(a / b)
DATA 1.5, 2.5
PRINT LOG(a + b)
