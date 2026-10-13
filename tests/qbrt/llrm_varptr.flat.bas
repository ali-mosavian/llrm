' VARPTR$ and SADD, and what DRAW and PLAY do with them: = takes a variable's
' value, X runs a string variable.  BC hangs on a DRAW or PLAY given a VARPTR$
' string under DOSBox, so this program checks itself and test_qbbc compares its
' output with the expectations below.  m32 (a VARPTR$ string is 5 bytes: the type and the
' 32-bit flat address; DEF SEG is ignored there, so the program has none).
' expect: 5 2 20 4 8 3
' expect: -1 -1
' expect: draw-1 -1 -1
' expect: x-1
' expect: play 1
DEFINT A-Z
DIM g!, d#, l&
OPEN "L.TXT" FOR OUTPUT AS #1
n = 30
l& = 12
g! = 7.5
d# = 3.4
a$ = "R10 D10 L10 U10"
v$ = VARPTR$(n)
PRINT #1, LEN(v$); ASC(LEFT$(v$, 1)); ASC(VARPTR$(l&)); ASC(VARPTR$(g!)); ASC(VARPTR$(d#)); ASC(VARPTR$(a$))
PRINT #1, PEEK(SADD(a$)) = ASC(a$); PEEK(SADD(a$) + 4) = ASC("D")
SCREEN 13
DRAW "BM10,10 C2 R30 D12 L3 U8"
DRAW "BM110,10 C2 R=" + VARPTR$(n) + "; D=" + VARPTR$(l&) + "; L=" + VARPTR$(d#) + "; U=" + VARPTR$(g!) + ";"
same1 = -1
FOR y = 0 TO 20
  FOR x = 0 TO 50
    IF POINT(10 + x, 10 + y) <> POINT(110 + x, 10 + y) THEN same1 = 0
  NEXT
NEXT
DRAW "BM10,60 C3 R10 D10 L10 U10"
DRAW "BM110,60 C3 X" + VARPTR$(a$) + ";"
same2 = -1
FOR y = 0 TO 20
  FOR x = 0 TO 20
    IF POINT(10 + x, 60 + y) <> POINT(110 + x, 60 + y) THEN same2 = 0
  NEXT
NEXT
b$ = "C4 U5 R5"
DRAW "BM10,100 X" + VARPTR$(b$) + "; X" + VARPTR$(a$) + ";"
DRAW "BM110,100 C4 U5 R5 R10 D10 L10 U10"
same3 = -1
FOR y = -6 TO 20
  FOR x = 0 TO 30
    IF POINT(10 + x, 100 + y) <> POINT(110 + x, 100 + y) THEN same3 = 0
  NEXT
NEXT
PRINT #1, "draw"; same1; same2; same3
PRINT #1, "x"; same2
PLAY "MB T255 L=" + VARPTR$(n) + "; C"
PRINT #1, "play"; PLAY(0)
SOUND 440, 0
CLOSE #1
END
