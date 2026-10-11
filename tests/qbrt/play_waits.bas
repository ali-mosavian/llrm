' When PLAY returns, to the nearest half second: a foreground PLAY once its last note has begun, then the rest
' of the music; a background PLAY at once; tempo, octave, length and style staying set from one PLAY to the next.
DEFINT I-N
PLAY "MF T120 L4 O3"
t! = TIMER
PLAY "C C C C"
a! = TIMER - t!
PLAY "P64"
b! = TIMER - t!
PRINT INT(a! * 2 + .5) / 2; INT(b! * 2 + .5) / 2
PLAY "MB T240 L8"
t! = TIMER
PLAY "C C C C C C C C"
a! = TIMER - t!
PLAY "MF P64"
b! = TIMER - t!
PRINT INT(a! * 2 + .5) / 2; INT(b! * 2 + .5) / 2
PLAY "T120 L4 MF"
t! = TIMER
PLAY "C"
PLAY "D"
PLAY "E"
PLAY "F"
a! = TIMER - t!
PLAY "P64"
b! = TIMER - t!
PRINT INT(a! * 2 + .5) / 2; INT(b! * 2 + .5) / 2
PLAY "ML"
t! = TIMER
PLAY "C D"
PLAY "T255 L64 P64"
a! = TIMER - t!
PRINT INT(a! * 2 + .5) / 2
PLAY "MS C D"
PLAY "T255 L64 P64"
