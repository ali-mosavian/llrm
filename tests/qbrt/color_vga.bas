' COLOR with a background in SCREEN 12 is an Illegal function call, as in BCOM45; the runtime accepted it.
SCREEN 12
ON ERROR GOTO bad
COLOR 12, 1
SCREEN 0
PRINT "accepted"
END
bad:
SCREEN 0
PRINT "error"; ERR
END
