' Graphics primitives and text in one screen mode: each segment prints its emulated milliseconds to the file named below.
' @MODE@ and @RESULT@ are filled in by gfxbench.py.
DEFINT A-Z
DECLARE SUB Mark (label$, started!)
DIM sprite(1 TO 700)
OPEN "@RESULT@" FOR OUTPUT AS #1
SCREEN @MODE@
SELECT CASE @MODE@
CASE 0: w = 80: h = 25: top = 15: cols = 80
CASE 1: w = 320: h = 200: top = 3: cols = 40
CASE 2: w = 640: h = 200: top = 1: cols = 80
CASE 7, 13: w = 320: h = 200: top = 15: cols = 40
CASE 8: w = 640: h = 200: top = 15: cols = 80
CASE 9: w = 640: h = 350: top = 15: cols = 80
CASE ELSE: w = 640: h = 480: top = 15: cols = 80
END SELECT
IF @MODE@ = 13 THEN top = 255
IF @MODE@ = 0 THEN GOTO textonly

t! = TIMER
FOR i = 1 TO 30000
    PSET (i MOD w, (i * 7) MOD h), i AND top
NEXT
Mark "pset", t!

t! = TIMER
s& = 0
FOR i = 1 TO 30000
    s& = s& + POINT(i MOD w, (i * 7) MOD h)
NEXT
Mark "point", t!

t! = TIMER
FOR i = 1 TO 120
    LINE (0, i MOD h)-(w - 1, h - 1 - i MOD h), i AND top
NEXT
Mark "line-long", t!

t! = TIMER
FOR i = 1 TO 1500
    LINE (i MOD (w - 20), i MOD (h - 20))-(i MOD (w - 20) + 15, i MOD (h - 20) + 9), i AND top
NEXT
Mark "line-short", t!

t! = TIMER
FOR i = 1 TO 150
    bx = i MOD (w - 40): by = i MOD (h - 40)
    LINE (bx, by)-(bx + 31, by + 31), i AND top, BF
NEXT
Mark "box-filled", t!

t! = TIMER
FOR i = 1 TO 50
    CIRCLE (w \ 2 + (i MOD 5) * 3, h \ 2), 40 + i MOD 7, i AND top
NEXT
Mark "circle", t!

t! = TIMER
FOR i = 1 TO 8
    LINE (20, 20)-(120, 80), top, B
    PAINT (60, 50), 1, top
    LINE (20, 20)-(120, 80), 0, BF
NEXT
Mark "paint", t!

t! = TIMER
FOR i = 1 TO 1000
    GET (10, 10)-(25, 25), sprite
    PUT (i MOD (w - 20), i MOD (h - 20)), sprite, XOR
NEXT
Mark "get-put", t!

textonly:
t! = TIMER
FOR i = 1 TO 300
    LOCATE 1 + i MOD 20, 1
    PRINT STRING$(cols - 1, 65 + i MOD 26)
NEXT
Mark "print-line", t!

t! = TIMER
FOR i = 1 TO 200
    LOCATE 24, 1
    PRINT "scrolling line"; i
NEXT
Mark "print-scroll", t!

t! = TIMER
FOR i = 1 TO 600
    LOCATE 1 + i MOD 20, 1 + (i * 3) MOD (cols - 12)
    PRINT "Hello, world"
NEXT
Mark "print-short", t!

CLOSE #1
END

SUB Mark (label$, started!)
    elapsed& = CLNG((TIMER - started!) * 1000)
    CLOSE #1
    OPEN "@RESULT@" FOR APPEND AS #1
    PRINT #1, label$; " "; elapsed&
END SUB
