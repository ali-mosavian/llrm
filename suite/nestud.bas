' A TYPE whose field is itself another TYPE, plain and arrayed, at module
' and procedure scope -- udt.bas only measured a flat structure. Solo is a
' single-field TYPE used only as a bare local, to see if its struct record
' ends up last in $$TYPES when nothing references it afterward.
DEFINT A-Z
TYPE Inner
    amt AS LONG
END TYPE

TYPE Outer
    part AS Inner
    tag AS STRING * 2
END TYPE

TYPE Solo
    v AS LONG
END TYPE

DIM o AS Outer
DIM arr(1) AS Outer

o.part.amt = 111
o.tag = "AB"
arr(0).part.amt = 1
arr(0).tag = "CD"
arr(1).part.amt = 2
arr(1).tag = "EF"

PRINT o.part.amt
PRINT o.tag
PRINT arr(0).part.amt; arr(1).part.amt
PRINT arr(0).tag
PRINT arr(1).tag

CALL Inside

DIM lastvar AS Solo
lastvar.v = 999
PRINT lastvar.v
PRINT "DONE"

SUB Inside
    DIM lo AS Outer
    DIM larr(1) AS Outer
    lo.part.amt = 333
    lo.tag = "GH"
    larr(0).part.amt = 4
    larr(0).tag = "IJ"
    larr(1).part.amt = 5
    larr(1).tag = "KL"
    PRINT lo.part.amt
    PRINT lo.tag
    PRINT larr(0).part.amt; larr(1).part.amt
    PRINT larr(0).tag
    PRINT larr(1).tag
END SUB
