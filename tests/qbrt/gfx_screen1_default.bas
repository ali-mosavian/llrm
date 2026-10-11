' SCREEN 1 before any COLOR: the colours the BIOS leaves, then each palette and background.
SCREEN 1
FOR c% = 0 TO 3
  LINE (10 + c% * 40, 10)-(40 + c% * 40, 50), c%, BF
NEXT
COLOR 4, 0
FOR c% = 0 TO 3
  LINE (10 + c% * 40, 60)-(40 + c% * 40, 100), c%, BF
NEXT
COLOR 9, 1
FOR c% = 0 TO 3
  LINE (10 + c% * 40, 110)-(40 + c% * 40, 150), c%, BF
NEXT
COLOR 2, 2
LOCATE 20, 5: PRINT "palette 2"
SLEEP
