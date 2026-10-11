' LINE INPUT takes the whole line as typed, with and without a prompt.
CLS
LINE INPUT "Name: "; a$
PRINT "["; a$; "]"
LINE INPUT b$
PRINT "["; b$; "]"
LINE INPUT ; c$
PRINT "["; c$; "]"
LOCATE 25, 1
