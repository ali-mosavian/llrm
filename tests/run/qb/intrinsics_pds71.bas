' Inlined LEN, ASC and MID$ read what the runtime's own routines read: a string's bytes, a
' MID$ that ends inside, at or past the string, and results the runtime frees (a concatenation,
' LEFT$) which keep the call. A loop over ASC(MID$()) once made a string temporary per byte.
' dialect: pds71
DEFINT A-Z
DECLARE FUNCTION Sum& (s AS STRING)
DECLARE FUNCTION Edge% (s AS STRING, i AS INTEGER, n AS INTEGER)
DIM p AS STRING, e AS STRING

p = "Oliver|Scrooge"
PRINT "SUM="; Sum&(p)
PRINT "LEN="; LEN(p); LEN(e)
PRINT "MID="; Edge%(p, 1, 3); Edge%(p, 13, 5); Edge%(p, 14, 1); Edge%(p, 15, 1); Edge%(p, 99, 1); Edge%(p, 5, 0)
PRINT "TEMP="; ASC(LEFT$(p, 2)); ASC(p + "x"); LEN(p + "ab"); LEN(MID$(p + "ab", 3, 4))
PRINT "ONE="; ASC(MID$(p, 2, 1)); ASC(MID$(p, 14, 9)); ASC(p)

FUNCTION Edge% (s AS STRING, i AS INTEGER, n AS INTEGER)
    Edge% = LEN(MID$(s, i, n))
END FUNCTION

FUNCTION Sum& (s AS STRING)
    DIM i AS INTEGER, t AS LONG
    FOR i = 1 TO LEN(s)
        t = t + ASC(MID$(s, i, 1))
    NEXT
    Sum& = t
END FUNCTION
