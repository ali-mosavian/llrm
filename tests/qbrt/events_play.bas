' ON PLAY(n) GOSUB: the handler runs when the notes waiting in the
' background music fall from n to n - 1; PLAY(0) counts the notes waiting.
DEFINT A-Z
DIM SHARED refills
OPEN "L.TXT" FOR OUTPUT AS #1
ON PLAY(2) GOSUB refill
PLAY ON
PLAY "MB T120 L4 O3 C D E F"
t! = TIMER
DO WHILE TIMER - t! < .2
LOOP
PRINT #1, "waiting"; PLAY(0)
DO WHILE TIMER - t! < 4
LOOP
PLAY OFF
PRINT #1, "refills"; refills; "waiting"; PLAY(0)
CLOSE #1
END

refill:
  refills = refills + 1
  IF refills <= 2 THEN PLAY "MB C"
  RETURN
