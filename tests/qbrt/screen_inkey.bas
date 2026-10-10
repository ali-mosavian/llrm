' INKEY$ with nothing typed is empty; TIMER is the seconds since midnight and
' only moves forward.
CLS
k$ = INKEY$
PRINT "["; k$; "]"; LEN(k$)
t! = TIMER
u! = TIMER
PRINT t! > 0; u! >= t!; t! < 86400
LOCATE 25, 1
