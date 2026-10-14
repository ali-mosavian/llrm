' DRAW's `=` and X take a variable by its VARPTR$ string (rt/mclprc.asm B$VALSCN, B$MCLXEQ).  The VARPTR$ string ends
' the command string: BC raises Illegal function call when more text follows it.
' Each line: the lit pixels of row 100 from column 95 to 150.
DEFINT A-Z
OPEN "L.TXT" FOR OUTPUT AS #1
SCREEN 1
n = 20
CLS
DRAW "BM100,100;C3;R=" + VARPTR$(n)
GOSUB count
a$ = "R10"
b$ = "R5X" + VARPTR$(a$)
CLS
DRAW "BM100,100;C3;X" + VARPTR$(b$)
GOSUB count
d! = 7.6
CLS
DRAW "BM100,100;C3;R=" + VARPTR$(d!)
GOSUB count
l& = 12
CLS
ON ERROR GOTO oops
DRAW "BM100,100;C3;R=" + VARPTR$(l&)
PRINT #1, "long"; e
CLOSE #1
END
oops:
e = ERR
RESUME NEXT
count:
c = 0
FOR x = 95 TO 150
  IF POINT(x, 100) <> 0 THEN c = c + 1
NEXT
PRINT #1, c
RETURN
