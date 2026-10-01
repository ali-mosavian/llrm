' BC adds or subtracts in place into a scalar INTEGER (x = x + e, x = e + x, x = x - e): the
' wrapped result is stored, then error 6; any other target keeps its old value.
DEFINT A-Z
ON ERROR GOTO h
x = 32767: y = 1: z = 7
z = x + y: PRINT "z=x+y"; z
z = 7: x = 32767: x = y + x: PRINT "x=y+x"; x
z = 7: x = 32767: x = x + 1 + 0: PRINT "x=x+1+0"; x
z = 7: x = 32767: x = x + y + 0: PRINT "x=x+y+0"; x
x = 32767: x = (x + y): PRINT "x=(x+y)"; x
x = -32768: z = 7: z = x - y: PRINT "z=x-y"; z
x = -32768: x = x - y: PRINT "x=x-y"; x
x = -32768: y = 1: x = y - x: PRINT "x=y-x"; x
DIM a(3)
a(1) = 32767: a(1) = a(1) + y: PRINT "a=a+y"; a(1)
x = 32767: x = x + y * 1: PRINT "x=x+y*1"; x
x = 32767: x = x + 1: PRINT "x=x+1"; x
x = 32767: x = x - -1: PRINT "x=x--1"; x
x = 32767: x = x + y: PRINT "x=x+y"; x
END
h:
PRINT "err"; ERR; ERL
RESUME NEXT
