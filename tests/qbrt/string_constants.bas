' CONST names of string literals are used where a string is: BC substitutes the text
DEFINT A-Z
CONST ROOT = "TWH", DOT$ = ".", FULL = "TWH.TMP"
DIM s AS STRING * 6
a$ = ROOT
PRINT a$; LEN(ROOT); LEN(FULL)
PRINT ROOT + DOT$ + "X"; FULL
s = ROOT
PRINT "["; s; "]"
IF a$ = ROOT THEN PRINT "same"
PRINT UCASE$(FULL); MID$(FULL, 2, 3)
OPEN FULL FOR OUTPUT AS #1
CLOSE #1
KILL FULL
PRINT "done"
