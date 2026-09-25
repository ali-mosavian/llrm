' One array element, addressed three times in one statement, and stored
' then reloaded twice. The index computation is the invariant worth
' hoisting and the reload is what a store-to-load forward removes.
DEFINT A-Z
DIM a(20)
DIM i, t
t = 0
FOR i = 1 TO 20
    a(i) = i * 3
    t = t + a(i) + a(i)
NEXT i
PRINT "T="; t
PRINT "DONE"
