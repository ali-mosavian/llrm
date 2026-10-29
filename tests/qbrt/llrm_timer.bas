' TIMER on DOS stepped with the BIOS tick, 55 ms (INT 21h function 2Ch), so every wait and every frame paced by it was 55 ms
' coarse. It reads the timer chip as well, and a tick that is counted late must not make it step back: one read in 60 did, by 55 ms.
' expect: backwards 0
' expect: finer -1
DEFINT A-Z
OPEN "L.TXT" FOR OUTPUT AS #1
t# = TIMER
smallest# = 99
FOR i = 1 TO 60
  DO: n# = TIMER: LOOP WHILE n# = t#
  d# = n# - t#
  IF d# < 0 THEN backwards = backwards + 1
  IF d# > 0 AND d# < smallest# THEN smallest# = d#
  t# = n#
NEXT i
PRINT #1, "backwards"; backwards
PRINT #1, "finer"; " "; (smallest# < .02)
CLOSE #1
END
