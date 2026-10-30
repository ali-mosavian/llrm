DECLARE SUB Fill ()
DIM SHARED lo AS INTEGER, up AS INTEGER
DIM SHARED a(lo TO up) AS INTEGER
lo = 2: up = 5
REDIM a(lo TO up) AS INTEGER
Fill
PRINT LBOUND(a), UBOUND(a), a(2) + a(5)
n% = 3
DIM b(n%) AS INTEGER
b(3) = 7
PRINT UBOUND(b), b(3)

SUB Fill
    a(2) = 20: a(5) = 22
END SUB
