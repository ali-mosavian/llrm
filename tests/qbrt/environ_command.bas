' COMMAND$ and ENVIRON$ had no entries, so a program using them did not link.
PRINT "["; COMMAND$; "]"
PRINT "["; ENVIRON$("NOSUCHVARIABLE"); "]"
a$ = ENVIRON$(1)
PRINT LEN(a$) > 0; INSTR(a$, "=") > 1
PRINT "["; ENVIRON$(200); "]"
PRINT ENVIRON$("path") = MID$(ENVIRON$(1), 6)
