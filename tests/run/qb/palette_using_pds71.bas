' dialect: pds71
' PALETTE USING: attribute n takes element n of a LONG array (QB rt/gwpal.asm); -1 leaves one alone, and the
' DAC is read back. An array with fewer elements than colours and a value past 63 are illegal function calls.
DEFINT A-Z
DIM p(0 TO 255) AS LONG, q(0 TO 3) AS LONG
SCREEN 13
FOR i = 0 TO 255
  p(i) = (i AND 63) + 256& * ((i * 3) AND 63) + 65536 * ((i * 5) AND 63)
NEXT
p(7) = -1
OUT &H3C8, 7
OUT &H3C9, 1: OUT &H3C9, 2: OUT &H3C9, 3
PALETTE USING p(0)
ON ERROR GOTO h
FOR i = 0 TO 255 STEP 51
  OUT &H3C7, i
  a = INP(&H3C9): b = INP(&H3C9): c = INP(&H3C9)
  PRINT i; a; b; c
NEXT
OUT &H3C7, 7
a = INP(&H3C9): b = INP(&H3C9): c = INP(&H3C9)
PRINT "kept"; a; b; c
PALETTE USING p(200)
PALETTE USING q(0)
p(9) = 64
PALETTE USING p(0)
SCREEN 0
END
h:
PRINT "caught"; ERR
RESUME NEXT
