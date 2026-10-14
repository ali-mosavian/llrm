' INPUT$: characters from the keyboard (here the redirected input, "ABC12345\r") and from a file, with and without the #.
a$ = INPUT$(3)
PRINT a$; LEN(a$)
b$ = INPUT$(2)
PRINT b$
OPEN "INPUTF.TXT" FOR OUTPUT AS #1
PRINT #1, "hello world"
CLOSE #1
OPEN "INPUTF.TXT" FOR INPUT AS #1
PRINT INPUT$(5, #1); "|"; INPUT$(1, 1); "|"; INPUT$(0, #1); "|"
c$ = INPUT$(5, #1)
PRINT c$; EOF(1)
CLOSE #1
ON ERROR GOTO oops
OPEN "INPUTF.TXT" FOR INPUT AS #1
d$ = INPUT$(200, #1)
PRINT "after"; e%
CLOSE #1
END
oops:
e% = ERR
RESUME NEXT
