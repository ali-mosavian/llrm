' SOUND frequency, ticks: in foreground music a note waits for the one
' sounding to end, in background music (PLAY "MB") it joins the queue.
' PLAY(0) counts the notes waiting.
DEFINT A-Z
OPEN "L.TXT" FOR OUTPUT AS #1
' Start on a clock tick: how long the program took to load decided which side of the first tick
' the first note fell, so one more relocation in the runtime flipped the first line.
t! = TIMER
DO WHILE TIMER = t!
LOOP
t! = TIMER
FOR i = 1 TO 4
  SOUND 440, 18
  PRINT #1, i; INT((TIMER - t!) + .3); PLAY(0)
  u! = TIMER
  DO WHILE TIMER - u! < .2
  LOOP
NEXT
SOUND 440, 0
PRINT #1, "stopped"; PLAY(0)
PLAY "MB"
t! = TIMER
FOR i = 1 TO 4
  SOUND 440, 18
  PRINT #1, i; INT((TIMER - t!) + .3); PLAY(0)
NEXT
SOUND 440, 0
ON ERROR GOTO oops
SOUND 20, 5
SOUND 440, -1
SOUND 440, 70000
CLOSE #1
END

oops:
  PRINT #1, "error"; ERR
  RESUME NEXT
