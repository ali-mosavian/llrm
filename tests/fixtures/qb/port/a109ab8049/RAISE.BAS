DEFINT A-Z
DECLARE SUB Check (n)
Check 3
SUB Check (n)
  IF n >= 0 THEN
    PRINT n
  ELSE
    ERROR 5
  END IF
END SUB
