' SHELL runs the interpreter on a command, then the program goes on; a bare array survives the memory it gave up
DEFINT A-Z
DIM a(1 TO 500) AS LONG
FOR i = 1 TO 500: a(i) = i * 3: NEXT
s$ = "kept"
SHELL "ECHO from the shell> QBSHELL.TMP"
OPEN "QBSHELL.TMP" FOR INPUT AS #1
LINE INPUT #1, t$
CLOSE #1
KILL "QBSHELL.TMP"
PRINT "[" + t$ + "]"
t& = 0
FOR i = 1 TO 500: t& = t& + a(i): NEXT
PRINT t&; s$
SHELL "NOSUCHPROGRAM"
PRINT "done"
