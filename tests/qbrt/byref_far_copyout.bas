' An element of a far array passed by reference: BC copies it into a near slot and back out after the call.
' Touch writes the same element through the array during the call, and the copy-out puts the old value back.
' SumTo only reads, and recurses on an element of the array it was given: its copy-outs change nothing.
DEFINT A-Z
DECLARE SUB Touch (x AS INTEGER)
DECLARE FUNCTION SumTo& (a() AS INTEGER, at AS INTEGER)
REDIM SHARED t(7) AS INTEGER

t(1) = 5
Touch t(1)
PRINT t(1)
t(1) = 2: t(2) = 3: t(3) = 0
PRINT SumTo&(t(), t(1))
PRINT t(1); t(2); t(3)

SUB Touch (x AS INTEGER)
    t(1) = 99
END SUB

FUNCTION SumTo& (a() AS INTEGER, at AS INTEGER)
    IF at = 0 THEN
        SumTo& = 0
    ELSE
        SumTo& = a(at) + SumTo&(a(), a(at))
    END IF
END FUNCTION
