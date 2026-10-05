' Scroll an 80x25 text screen up a row, blank the bottom row, scroll it back down, and keep a copy
' in a back buffer: fifty times, then a weighted sum of the copy.
DEFINT A-Z
DECLARE FUNCTION BenchScroll& ()
DIM SHARED scr(1999), back(1999)
FOR i = 0 TO 1999
  scr(i) = ((i * 7) AND 255) OR &H700
NEXT
PRINT LTRIM$(STR$(BenchScroll&))
END

FUNCTION BenchScroll&
  total& = 0
  FOR r = 0 TO 49
    FOR i = 0 TO 1919
      scr(i) = scr(i + 80)
    NEXT
    FOR i = 1920 TO 1999
      scr(i) = &H720
    NEXT
    FOR i = 1919 TO 0 STEP -1
      scr(i + 80) = scr(i)
    NEXT
    FOR i = 0 TO 1999
      back(i) = scr(i)
    NEXT
  NEXT
  FOR i = 0 TO 1999
    total& = total& + CLNG(back(i)) * ((i AND 15) + 1)
  NEXT
  BenchScroll& = total&
END FUNCTION
