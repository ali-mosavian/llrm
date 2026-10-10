' The music queue: how many notes a background PLAY takes before it makes the program wait, and that a
' foreground PLAY returns once its last note has begun.  Notes of about half a second; the time is printed to
' the nearest half second.
DIM n%(1 TO 8)
FOR i% = 1 TO 8: READ n%(i%): NEXT
DATA 1,5,10,16,20,26,32,40
FOR i% = 1 TO 8
  PLAY "MB T255 L8 P64"
  PLAY "MF P64"
  n$ = ""
  FOR k% = 1 TO n%(i%): n$ = n$ + "C ": NEXT
  s! = TIMER
  PLAY "MB " + n$
  e! = TIMER
  PRINT INT((e! - s!) * 2 + .5) / 2;
  PLAY "MF P64"
NEXT
PRINT
PLAY "MB T255 L8 O3 C"
s! = TIMER
PLAY "MF L4 D"
e! = TIMER
PRINT INT((e! - s!) * 2 + .5) / 2;
PLAY "MF P64"
