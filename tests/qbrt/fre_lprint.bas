' FRE and LPRINT had no entries, so a program using them did not link.  The sizes differ from BCOM45's
' memory map, so only their sanity is printed.
a$ = SPACE$(100)
b$ = a$
b$ = ""
PRINT FRE("") > 0; FRE(0) > 0; FRE(-1) > 0; FRE(-2) > 0
PRINT FRE("") <= 65535; FRE(-2) < 65535
LPRINT "to the printer"; 1
PRINT "after"
