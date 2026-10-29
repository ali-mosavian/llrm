' PLAY's music macro language: how many notes (and rests) each string queues,
' and which strings are errors.  PLAY(0) counts the notes waiting.  A rest is started first and given a tick, so the
' notes cannot start during the count: the first note leaves the queue at a clock tick that can fall between PLAY and PLAY(0).
DEFINT A-Z
DIM s$(10)
s$(0) = "P4P8P64"
s$(1) = "L8C.D..E"
s$(2) = "MS C MN D ML E"
s$(3) = "T120L4CDE"
s$(4) = "C4.D2..G16"
s$(5) = "O0C O6B"
s$(6) = "L64CDEFGAB"
s$(7) = "MBMFC"
s$(8) = "ABCDEFG"
s$(9) = "a b c"
s$(10) = ">>>C<<<C"
OPEN "L.TXT" FOR OUTPUT AS #1
FOR i = 0 TO 10
  PLAY "MB T255 P1"
  t! = TIMER
  DO WHILE TIMER - t! < .15
  LOOP
  PLAY "MB T255 L64 O2 " + s$(i)
  PRINT #1, i; PLAY(0)
  SOUND 440, 0
NEXT
PLAY "MB"
ON ERROR GOTO oops
PLAY "Z"
PLAY "O7"
PLAY "L65"
PRINT #1, "done"
SOUND 440, 0
CLOSE #1
END

oops:
  PRINT #1, "error"; ERR
  RESUME NEXT
