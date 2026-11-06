' flags: -O2 --isel=old
' dialect: vbdos qb45
' Two constants made before a loop that carries two values: re = 0, im = 0
' inside an outer loop. LICM made one `fldz` of both, and the x87 stack
' allocator had no definition for the second carried entry
' ("floating stack input is unavailable"). One line, then DONE.
DEFINT A-Z
DEFDBL X, Y, R, M, T
total = 0
FOR i = 0 TO 9
  X = i * .1
  Y = .3
  R = 0
  M = 0
  n = 0
  DO
    T = R * R - M * M
    M = R * M
    M = M + M + Y
    R = T + X
    n = n + 1
  LOOP UNTIL R * R + M * M >= 4 OR n = 50
  total = total + n
NEXT
PRINT total
PRINT "DONE"
