' flags: -O2 --cpu 486 -fsanitize=stack
' mask: [0-9A-F]{4}:[0-9A-F]{4}
' Same through the runtime's checking frame entry, B$ENRD.
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
