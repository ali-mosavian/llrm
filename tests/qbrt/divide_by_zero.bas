' An integer division by zero is the CPU's fault: the runtime takes interrupt 0 and ends the program with
' the error Division by zero, as BCOM45 does (it crashed before: no handler).
x% = 0
PRINT "before"
PRINT 5 \ x%
PRINT "not reached"
