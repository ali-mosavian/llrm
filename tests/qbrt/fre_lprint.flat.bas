' fre_lprint.bas at one address space: the free string space is not under 64K there (the heaps share a region of megabytes), so
' the comparisons with 65535, which only describe real mode's segments, are not made; the others print what they did.
a$ = SPACE$(100)
b$ = a$
b$ = ""
PRINT FRE("") > 0; FRE(0) > 0; FRE(-1) > 0; FRE(-2) > 0
PRINT FRE("") > 0; FRE(-2) > 0
LPRINT "to the printer"; 1
PRINT "after"
