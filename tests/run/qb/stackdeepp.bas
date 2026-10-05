' flags: -O2 --cpu 486 --own-frames -fsanitize=stack
' dialect: pds71
' A recursion that fits runs unchanged under -fsanitize=stack.
DECLARE FUNCTION Deep% (n AS INTEGER)
PRINT Deep%(50)
FUNCTION Deep% (n AS INTEGER)
  IF n = 0 THEN
    Deep% = 0
  ELSE
    Deep% = 1 + Deep%(n - 1)
  END IF
END FUNCTION
