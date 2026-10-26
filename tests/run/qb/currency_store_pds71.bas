' dialect: pds71
' CURRENCY storage: variables, arrays, UDT fields, parameters and results copy 8-byte scaled values; PRINT shows them.
DEFCUR D
TYPE Pair
  n AS INTEGER
  c AS CURRENCY
END TYPE
DECLARE SUB Show (v AS CURRENCY)
DECLARE SUB Bump (v AS CURRENCY)
DECLARE FUNCTION Twice@ (BYVAL v AS CURRENCY)
DIM SHARED s AS CURRENCY
DIM a AS CURRENCY, b AS CURRENCY
DIM t(1 TO 3) AS CURRENCY
DIM p AS Pair
a = 1.5
b = a
t(2) = b
p.n = 7
p.c = t(2)
d = 12345
s = -0.0001
PRINT a; b; t(2); p.n; p.c
PRINT d; s; 922337203685477.5807@
Show a
Show t(1)
CALL Bump(a)
PRINT a
PRINT Twice(a)
a = 1.00005: PRINT a;
a = -1.00005: PRINT a;
a = 0.00004: PRINT a;
a = 1.5E3: PRINT a;
a = 2: PRINT a;
a = -70000: PRINT a;
a = 0.1: PRINT a
END

SUB Show (v AS CURRENCY)
  PRINT "show"; v
END SUB

SUB Bump (v AS CURRENCY)
  v = 99.9999
END SUB

FUNCTION Twice@ (BYVAL v AS CURRENCY)
  Twice = v
END FUNCTION
