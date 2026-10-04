' ASC("") and a MID$ start outside the string raise error 5 under -fsanitize=bounds, in every
' dialect. The program handles errors, so each call is an invoke and the runtime raises it.
' dialect: pds71
' flags: -O2 --cpu 486 -fsanitize=bounds
DEFINT A-Z
DECLARE FUNCTION Probe% (s AS STRING, i AS INTEGER)
DIM caught AS INTEGER, p AS STRING, e AS STRING, v AS INTEGER, n AS INTEGER
ON ERROR GOTO handler

p = "abc"
caught = 0: v = 0
v = ASC(e)
PRINT "EMPTY="; caught; v
caught = 0: v = 0
v = Probe%(p, 0)
PRINT "ZERO="; caught; v
caught = 0: v = 0
v = Probe%(p, 4)
PRINT "PAST="; caught; v
caught = 0: v = 0
v = Probe%(p, 3)
PRINT "LAST="; caught; v
caught = 0: v = 0
v = ASC(MID$(p, 2, -1))
PRINT "NEG="; caught; v
caught = 0: v = 0
n = 256
v = ASC(CHR$(n))
PRINT "BIG="; caught; v
caught = 0: v = 0
n = -1
v = ASC(CHR$(n))
PRINT "NEGCHR="; caught; v
PRINT "DONE"
END

handler:
    caught = ERR
    RESUME NEXT

FUNCTION Probe% (s AS STRING, i AS INTEGER)
    Probe% = ASC(MID$(s, i, 1))
END FUNCTION
