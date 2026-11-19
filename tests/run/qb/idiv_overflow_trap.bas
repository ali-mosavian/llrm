' flags: -O0 | -O1 | -O2 | -Os
' mask: Division by zero.*
' INTEGER \ at m16 is the 16-bit idiv BC emits, with no check of ours for -32768 \ -1: the processor's divide fault reaches the runtime's
' handler and ends the program with error 11 (BC 4.50 reports the same, oracle ~/scratch/qbmeta/oracle).
DEFINT A-Z
READ a, b
DATA -32768, -1
PRINT "a ="; a; "b ="; b
PRINT "idiv:"; a \ b
PRINT "not reached"
