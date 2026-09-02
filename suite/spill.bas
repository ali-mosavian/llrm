' More variables than registers, and they are not equally worth keeping.
' h1, h2 and h3 are read a hundred times in the inner loop; o1 and o2 ten
' times in the outer. An allocator that spills by cost keeps the first
' three and spills the last two -- BC spills all of them, every
' statement.
DEFINT A-Z
DIM h1, h2, h3, o1, o2, t, i, j
h1 = 3
h2 = 5
h3 = 7
o1 = 0
o2 = 0
t = 0
FOR i = 1 TO 10
    o1 = o1 + i
    o2 = o2 + o1
    FOR j = 1 TO 10
        t = t + h1 * h2 + h3
    NEXT j
NEXT i
PRINT "T="; t
PRINT "O="; o2
PRINT "DONE"
