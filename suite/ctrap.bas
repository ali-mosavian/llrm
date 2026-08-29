' What C says about a long divide, and what qbopt emits for it.
'
' idiv traps twice where BC's runtime does not: on a zero divisor, and on
' -2147483648 \ -1 whose true answer does not fit. qbopt emits neither trap. It
' tests the divisor first: -1 is handled by negating, which gives the wrapping
' answer and cannot fault, and zero yields zero, which C leaves undefined and
' this defines.
'
' So the rewritten program deliberately disagrees with the one BC built: the
' baseline raises error 11 on a zero divisor and this does not. tools/e2e.py
' knows, and checks this one against the golden alone.
DEFINT A-Z
DIM a AS LONG, r AS LONG, zero AS LONG, low AS LONG, minus AS LONG
DIM caught AS INTEGER

ON ERROR GOTO handler

a = 305419896
zero = 0
caught = 0
r = a \ zero
PRINT "DIVZERO="; caught; r
caught = 0
r = a MOD zero
PRINT "MODZERO="; caught; r

low = -2147483647 - 1
minus = -1
caught = 0
r = low \ minus
PRINT "DIVEDGE="; caught; r
caught = 0
r = low MOD minus
PRINT "MODEDGE="; caught; r

PRINT "DONE"
END

handler:
caught = ERR
RESUME NEXT
