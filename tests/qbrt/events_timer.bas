' ON TIMER(n) GOSUB: an event every n seconds while the timer is on; handlers run between statements.
DEFINT A-Z
DIM SHARED ticks
OPEN "L.TXT" FOR OUTPUT AS #1
ON TIMER(1) GOSUB tick
TIMER ON
start! = TIMER
DO WHILE ticks < 3
LOOP
TIMER OFF
elapsed = INT(TIMER - start! + .5)
PRINT #1, "ticks"; ticks; "after"; elapsed; "seconds"
' off: nothing more arrives
before = ticks
t! = TIMER
DO WHILE TIMER - t! < 2.5
LOOP
PRINT #1, "still"; ticks - before
' stop keeps the events, ON delivers one
TIMER ON
TIMER STOP
t! = TIMER
DO WHILE TIMER - t! < 2.5
LOOP
PRINT #1, "stopped"; ticks - before
TIMER ON
PRINT #1, "on again"; ticks - before
CLOSE #1
END

tick:
  ticks = ticks + 1
  RETURN
