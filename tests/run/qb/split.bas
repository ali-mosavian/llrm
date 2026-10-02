' Two variables whose live ranges do not overlap: a is dead before b is
' born, so one register holds both. Splitting a live range is the other
' half of allocation, and BC has neither half.
DEFINT A-Z
DIM a, b, r, i
r = 0
a = 2
FOR i = 1 TO 10
    r = r + a * i
NEXT i
b = 3
FOR i = 1 TO 10
    r = r + b * i
NEXT i
PRINT "R="; r
PRINT "DONE"
