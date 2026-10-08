' flags: -O2 -march=i486 --own-frames -fsanitize=stack
' mask: [0-9A-F]{4}:[0-9A-F]{4}
' Unbounded recursion under -fsanitize=stack: BC /D's fatal "Out of stack space" rather than
' running off the stack into the heap. Without the check the program crashed.
DECLARE FUNCTION Deep% (n AS INTEGER)
PRINT "start"
PRINT Deep%(30000)
PRINT "unreachable"
FUNCTION Deep% (n AS INTEGER)
  IF n = 0 THEN
    Deep% = 0
  ELSE
    Deep% = 1 + Deep%(n - 1)
  END IF
END FUNCTION
