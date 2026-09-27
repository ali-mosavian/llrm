' ON ERROR at module level, raised by ERROR n and by runtime calls, with each
' RESUME form. The handler reads ERR; after RESUME, ERR is 0 again. Every error
' is raised by a call, which a rewrite can make an invoke: division by zero is
' C's (agents.md), and SQR(-1) raises nothing under /FPi.
DEFINT A-Z
DIM caught AS INTEGER, where AS INTEGER, k AS INTEGER, i AS INTEGER
DIM sum AS INTEGER, a AS LONG, s AS STRING

ON ERROR GOTO handler

where = 1
ERROR 52
PRINT "ERROR="; caught; ERR

where = 2
k = -1
s = SPACE$(k)
PRINT "SPACE="; caught

where = 3
k = 300
s = CHR$(k)
PRINT "RETRY="; caught; ASC(s)

where = 4
ERROR 7
PRINT "SKIPPED"
after4:
PRINT "LABEL="; caught

where = 5
sum = 0
FOR i = 1 TO 5
    s = LEFT$("abc", 3 - i)
    sum = sum + i
NEXT i
PRINT "LOOP="; caught; sum

a = 305419896
where = 6
ERROR 9
PRINT "KEPT="; a; caught

PRINT "DONE"
END

handler:
caught = ERR
SELECT CASE where
CASE 3
    k = 65
    RESUME
CASE 4
    RESUME after4
CASE ELSE
    RESUME NEXT
END SELECT
