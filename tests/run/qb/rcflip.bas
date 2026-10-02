' deedlines' plasmablobs builds its plasma ramp with a direction flag that
' flips at each end. The flag lives in a local, so every store to it but the
' last is dead once it is promoted.
DEFINT A-Z
DECLARE SUB ramp ()
DIM SHARED dt100%(-180 TO 180)
ramp
END
SUB ramp
rc% = -1
FOR i% = -1 TO -156 STEP -1
dt100%(i%) = 63 - (i% - ((i% - 63) \ 64) * 64)
IF rc% = 1 THEN dt100%(i%) = 63 - dt100%(i%)
IF dt100%(i%) = 0 AND rc% = 1 THEN rc% = -1
IF dt100%(i%) = 63 AND rc% = -1 THEN rc% = 1
NEXT i%
rc% = 1
FOR i% = 0 TO 156
dt100%(i%) = i% - ((i%) \ 64) * 64
IF rc% = -1 THEN dt100%(i%) = 63 - dt100%(i%)
IF dt100%(i%) = 63 AND rc% = 1 THEN rc% = -1
IF dt100%(i%) = 0 AND rc% = -1 THEN rc% = 1
NEXT i%
s& = 0
FOR i% = -156 TO 156
s& = s& + dt100%(i%) * (i% + 200)
NEXT i%
PRINT "SUM="; s&
PRINT "DONE"
END SUB
