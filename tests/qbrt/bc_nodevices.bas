' The joystick, the light pen, the serial port and user events of a machine
' that has none: the statements are taken and nothing happens.
DEFINT A-Z
DIM SHARED fired
OPEN "L.TXT" FOR OUTPUT AS #1
ON STRIG(0) GOSUB h
ON PEN GOSUB h
ON COM(1) GOSUB h
ON UEVENT GOSUB h
STRIG(0) ON
STRIG ON
PEN ON
COM(1) ON
UEVENT ON
t! = TIMER
DO WHILE TIMER - t! < .5
LOOP
PRINT #1, "fired"; fired; "pen"; PEN(0); PEN(3); "trig"; STRIG(0); STRIG(1)
PRINT #1, "stick"; STICK(0); STICK(1); STICK(2); STICK(3)
STRIG(0) STOP
STRIG OFF
PEN OFF
COM(1) OFF
UEVENT OFF
CLOSE #1
END

h:
  fired = fired + 1
  RETURN
