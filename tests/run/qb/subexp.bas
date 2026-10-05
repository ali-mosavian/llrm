' The same subexpression in two statements. BC compiles a statement at a
' time, so it computes x + y twice and reloads both operands for the
' second -- there is nothing here a common-subexpression pass would not
' take, and BC takes none of it.
DEFINT A-Z
DIM x, y, p, q
x = 11
y = 5
p = (x + y) * 2
q = (x + y) * 3
PRINT "P="; p
PRINT "Q="; q
PRINT "DONE"
