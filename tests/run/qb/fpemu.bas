' Under /FPi every floating-point operation is an int 34h..3Bh site: the
' emulator's interrupt where the ESC opcode would go, with the x87 operand
' following inline. The emulator patches those sites at run time, and the
' runtime pass could not move code out from under one that had already been
' patched. Here the moving happens before the program has ever run, so no site
' has been patched yet -- this program is what says whether that is enough.
'
' Every FP site below sits after a long divide, whose call site grows by three
' bytes when it is absorbed, so the pass moves them itself. tests/test_motion.py
' also runs this one with a nop inserted ahead of everything, which moves every
' site whatever the configuration does.
'
' The results are exact integers printed through CLNG. What is under test is
' that the emulated instruction still runs and still gets the right answer, not
' QuickBASIC's floating-point PRINT formatting.
DEFINT A-Z
DIM x AS LONG, y AS LONG, q AS LONG, r AS LONG, m AS LONG
DIM a AS DOUBLE, b AS DOUBLE, c AS DOUBLE
DIM s AS SINGLE, t AS SINGLE

x = 1073741831
y = 1024

q = x \ y
a = q
b = y

PRINT "DIV="; q
PRINT "FADD="; CLNG(a + b)
PRINT "FSUB="; CLNG(a - b)

r = x MOD y

PRINT "MOD="; r
PRINT "FMUL="; CLNG(a * b)
PRINT "FDIV="; CLNG(a / b)

c = b / 2048
PRINT "FHALF="; CLNG(c * 10)
PRINT "FSQR="; CLNG(SQR(a))

s = q
t = 4

PRINT "SADD="; CLNG(s + t)
PRINT "SMUL="; CLNG(s * t)

m = x AND 1073741824

PRINT "AND="; m
PRINT "FCMP="; (a > b); (b > a)

PRINT "DONE"
END
