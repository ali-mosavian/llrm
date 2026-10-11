' TIMER is the seconds since midnight and does not run backwards: on dos32 the high halves of CX and DX
' stayed in the time of day, and a delay loop calibrated on it ran 30 cells in two seconds.
t1 = TIMER
FOR i# = 1 TO 20000: NEXT i#
t2 = TIMER
PRINT t1 >= 0 AND t1 < 86400; t2 >= t1 AND t2 - t1 < 5
