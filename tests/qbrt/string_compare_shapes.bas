' String comparisons read as values, in conditions, selects and loops: B$SCMP returns the flags, which only a comparison may read.
DIM a$(3), b$(3)
a$ = "abc": b$ = "abd": c$ = "": d$ = "x"
x% = (a$ < b$) AND (c$ < d$)
y% = (a$ < b$) OR (c$ = d$)
z% = (a$ < b$) XOR (c$ <= d$)
n% = -(a$ <= b$)
s% = (a$ > b$) * 3 + (c$ <> d$) * 5
m% = -(a$ = b$) - (a$ <> b$)
IF a$ < b$ AND c$ < d$ THEN PRINT "both"
IF a$ < b$ OR c$ = d$ THEN PRINT "either"
IF a$ < b$ THEN
k% = 1
ELSEIF a$ = b$ THEN
k% = 2
ELSE
k% = 3
END IF
SELECT CASE a$
CASE "abc"
PRINT "is abc"
CASE IS > "b"
PRINT "big"
CASE ELSE
PRINT "other"
END SELECT
FOR i% = 1 TO 3
  a$(i%) = CHR$(64 + i%)
NEXT
j% = 1
WHILE a$(j%) < "C" AND j% < 3: j% = j% + 1: WEND
t% = a$ < b$
u% = t% + (a$(1) > a$(2)) + 2 * (a$ < b$)
r% = ((a$ < b$) = (c$ < d$))
w& = (a$ < b$) * 1000& + (c$ > d$)
PRINT x%; y%; z%; n%; s%; m%; k%; j%; t%; u%; r%; w&
