' A fill's queue is its own block on a flat target, not the free string space: with the strings holding nearly all of
' it, a fill that queues a seed for each of 200 stripes still covers the picture (it ended in Out of memory).
DEFINT A-Z
DIM keep$(1 TO 200)
SCREEN 9
FOR i = 1 TO 200
  IF FRE("") > 33000 THEN keep$(i) = SPACE$(30000)
NEXT
rest& = FRE("")
IF rest& > 400 THEN fill$ = SPACE$(rest& - 400)
FOR x = 3 TO 597 STEP 3
  LINE (x, 10)-(x, 300), 15
NEXT
PAINT (1, 320), 4, 15
PRINT POINT(4, 100); POINT(300, 100); POINT(301, 100); POINT(4, 5)
