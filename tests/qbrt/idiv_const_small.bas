' INTEGER \ 16 of a byte value, tested against 0 (the JPEG decoder's 16-bit table test).
DEFINT A-Z
FUNCTION Pick%
  Pick% = 7
END FUNCTION
DECLARE FUNCTION Pick%
t = Pick%
IF t \ 16 <> 0 THEN PRINT "wrong" ELSE PRINT "right"
t = 100
PRINT t \ 16; t MOD 16; (t - 120) \ 16
