' RUN: llrm-qb %s --dialect pds71 --runtime pds71 -O2 --cpu 486 -fsanitize=bounds --huge-arrays -S -o /dev/stdout
' /D checks each subscript in code and raises ERROR 9 itself: every element
' was a B$HARY call, which also hid the check from the optimizer.
' CHECK-NOT: B$HARY
' CHECK: B$SERR
' CHECK-NOT: B$HARY
DEFINT A-Z
DECLARE SUB Touch (lo%, hi%)
Touch 1, 30000
END

SUB Touch (lo, hi)
    DIM s(1 TO 10) AS LONG
    REDIM l(lo TO hi) AS LONG
    REDIM g(lo TO 200, -2 TO 199) AS INTEGER
    s(hi MOD 10) = 1: l(hi) = 2: g(lo, -2) = 3
    PRINT s(hi MOD 10); l(hi); g(lo, -2)
END SUB
