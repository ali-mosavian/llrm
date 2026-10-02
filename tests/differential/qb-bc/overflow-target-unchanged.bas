' bc: /O /E /X /D
' flags: -ftrapv
' diverges: BC stores the wrapped sum first
' On an INTEGER overflow llrm leaves the target unchanged, whatever the statement's shape.
' BC /D stores the wrapped sum first for x = x + e and x = x - e (it adds into the variable):
' docs/frontends/qb/divergences.md.
DEFINT A-Z
DIM z(3)
ON ERROR GOTO h
x = 32767: y = 1: z(1) = 32767
x = x + y: PRINT "x=x+y"; x
x = 32767: x = y + x: PRINT "x=y+x"; x
x = 32767: x = x + 1: PRINT "x=x+1"; x
x = 32767: x = x + y + 0: PRINT "x=x+y+0"; x
x = -32768: x = x - y: PRINT "x=x-y"; x
z(1) = z(1) + y: PRINT "z=z+y"; z(1)
END
h:
PRINT "err"; ERR
RESUME NEXT
