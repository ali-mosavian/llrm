' INTEGER \ at -32768 by -1: BC emits the bare 16-bit idiv, the processor's divide fault reaches the runtime's handler and ends the
' program with error 11, Division by zero. Ours leaves the check out where the target traps it (Target::idiv_overflow_traps).
DEFINT A-Z
READ a, b
DATA -32768, -1
PRINT "a ="; a; "b ="; b
PRINT "idiv:"; a \ b
PRINT "not reached"
