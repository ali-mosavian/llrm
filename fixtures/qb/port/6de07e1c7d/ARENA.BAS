DEFINT A-Z
DIM arena(1 TO 50, 1 TO 80)
FOR row = 1 TO 50
  FOR col = 1 TO 80
    arena(row, col) = row + col
  NEXT col
NEXT row
PRINT arena(3, 4)
