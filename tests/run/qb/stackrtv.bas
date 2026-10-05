' flags: -O2 --cpu 486 -fsanitize=stack
' dialect: vbdos
' mask: [0-9A-F]{4}:[0-9A-F]{4}
' Unbounded recursion in a procedure the runtime frames (its local STRING needs the runtime's frame)
' enters the runtime's checking entry B$ENRD under -fsanitize=stack: the same fatal "Out of stack
' space" as an own frame's check, not a crash.
DECLARE SUB Deep (n AS INTEGER)
PRINT "start"
Deep 30000
PRINT "unreachable"
SUB Deep (n AS INTEGER)
  DIM t AS STRING
  t = "x"
  IF n > 0 THEN Deep n - 1
END SUB
