' The MID$ statement inline: a byte, a run, a start or count the string does not allow (error 5 before anything is copied), a source
' shorter than the count, a fixed-length string, and one string over itself.
DEFINT A-Z
DECLARE SUB Try (what AS STRING)
DIM SHARED n AS INTEGER
DIM SHARED s AS STRING
DIM SHARED f AS STRING * 6
DIM SHARED d AS STRING
ON ERROR GOTO Caught

s = "abcdef": f = "abcdef"
FOR i = 1 TO 6
    MID$(s, i, 1) = CHR$(48 + i)
    MID$(f, i, 1) = CHR$(64 + i)
NEXT
PRINT s; "|"; f
MID$(s, 2, 3) = "xyz123": PRINT s
MID$(s, 5) = "QRSTUV": PRINT s
MID$(f, 3, 9) = "pq": PRINT f
MID$(s, 3, 2) = "": PRINT s
MID$(s, 1, 0) = "ZZ": PRINT s
d = "overlap!"
MID$(d, 2) = d: PRINT d
Try "start 0"
MID$(s, 0, 1) = "x"
Try "start 7"
MID$(s, 7, 1) = "x"
Try "start 7 fixed"
MID$(f, 7, 1) = "x"
Try "count -1"
MID$(s, 1, -1) = "x"
Try "empty target"
d = ""
MID$(d, 1, 1) = "x"
Try "after"
PRINT s; "|"; f; "|"; d
END

Caught:
PRINT "error"; ERR
n = 1
RESUME NEXT

SUB Try (what AS STRING)
    PRINT what
END SUB
