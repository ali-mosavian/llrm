' A NUL in a PRINTed string prints nothing under BCOM45 (the console driver drops it); the C runtime wrote the byte.
PRINT "a"; CHR$(0); "b"
PRINT STRING$(3, 0); "x"
PRINT LEN(CHR$(0)); "|"; LEN("a" + CHR$(0))
