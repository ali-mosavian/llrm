' flags: -O1 | -O2 | -O3 | -Os
' LOG and EXP over digit boundaries: the runtime multiplied by ln2 and log2e as doubles; BC loads FLDLN2/FLDL2E (80-bit), so 3 of 8 lines differed in the last digit. .out is real BC's text.
DIM x AS DOUBLE
DATA 10, 3, 2, 0.5, 7.25, 123.456, 1E-5, 99999
FOR i = 1 TO 8
READ x
PRINT LOG(x); EXP(x / 10#); LOG(x * 3.7#); EXP(-x / 100#)
NEXT
