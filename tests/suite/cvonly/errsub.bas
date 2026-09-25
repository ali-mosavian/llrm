' An inline ON GOTO table, an ON ERROR GOTO/RESUME map, and a real SUB all in
' one module -- the untested combination a design review flagged: does a
' table's own fallthrough still land in the right body when a procedure's
' bytes sit between the table and whatever comes after it?
DEFINT A-Z
DECLARE SUB Report (n AS LONG)

DIM a AS LONG, caught AS INTEGER
DIM zero AS LONG
DIM choice AS INTEGER

ON ERROR GOTO handler

choice = 2
ON choice GOTO l1, l2, l3

l1:
PRINT "ONE"
GOTO cont

l2:
PRINT "TWO"
GOTO cont

l3:
PRINT "THREE"

cont:
a = 7
CALL Report(a)
a = a \ zero
PRINT "UNREACHED"

PRINT "DONE"
END

handler:
caught = ERR
RESUME NEXT

SUB Report (n AS LONG)
    PRINT "N="; n
END SUB
