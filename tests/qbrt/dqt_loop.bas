' The JPEG decoder's table loop: a marker's length counts down by 65 per table, a table byte tested with \ 16.
DEFINT A-Z
DECLARE FUNCTION NextByte% ()
DECLARE FUNCTION Read16% ()
DECLARE FUNCTION Decode& ()
DIM SHARED quant(0 TO 3, 0 TO 63) AS INTEGER
DIM SHARED cur AS INTEGER
PRINT Decode&

FUNCTION NextByte%
  cur = cur + 1
  IF cur = 1 THEN NextByte% = 219 ELSE IF cur = 66 + 3 THEN NextByte% = 255 ELSE NextByte% = (cur * 7) AND 15
END FUNCTION

FUNCTION Read16%
  Read16% = 65 + 2
END FUNCTION

FUNCTION Decode&
  DIM marker AS INTEGER, length AS INTEGER, t AS INTEGER, k AS INTEGER
  marker = NextByte%
  length = Read16% - 2
  IF marker = &HDB THEN
    DO WHILE length > 0
      t = NextByte%
      IF t \ 16 <> 0 THEN
        PRINT "wide table"
        Decode& = 0
        EXIT FUNCTION
      END IF
      FOR k = 0 TO 63
        quant(t AND 3, k) = NextByte%
      NEXT
      length = length - 65
    LOOP
  END IF
  Decode& = quant(1, 5) + 100& * cur
END FUNCTION
