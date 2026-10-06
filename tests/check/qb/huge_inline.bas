' RUN: llrm-qb %s --dialect pds71 --runtime pds71 -O2 -march=i486 --huge-arrays -S -o /dev/stdout
' /AH addresses every element inline: B$HARY was called for each, 1D and 2D,
' every element size, known bounds or not.
' CHECK: B$RDIM
' CHECK-NOT: B$HARY
' CHECK: retf
DEFINT A-Z
TYPE Pair
    x AS LONG
    y AS LONG
END TYPE
DECLARE SUB Touch (lo%, hi%)
Touch 1, 30000
END

SUB Touch (lo, hi)
    REDIM w(lo TO hi) AS INTEGER
    REDIM l(lo TO hi) AS LONG
    REDIM p(lo TO hi) AS Pair
    REDIM d(lo TO hi) AS DOUBLE
    REDIM s(lo TO hi) AS STRING * 3
    REDIM g(lo TO 200, -2 TO 199) AS LONG
    w(hi) = 1: l(hi) = 2: p(hi).y = 3: d(hi) = 4: s(hi) = "5": g(lo, -2) = 6
    PRINT w(hi); l(hi); p(hi).y; d(hi); s(hi); g(lo, -2)
END SUB
