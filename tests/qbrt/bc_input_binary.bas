' INPUT$ past the end of a file: Input past end for a sequential one, the shorter string for a BINARY one (rt/inputf.asm).
OPEN "L.TXT" FOR OUTPUT AS #1
OPEN "IB.DAT" FOR OUTPUT AS #2
PRINT #2, "hello";
CLOSE #2
OPEN "IB.DAT" FOR BINARY AS #2
a$ = INPUT$(8, #2)
PRINT #1, LEN(a$); "["; a$; "]"
b$ = INPUT$(3, #2)
PRINT #1, LEN(b$); EOF(2)
CLOSE #2
ON ERROR GOTO oops
OPEN "IB.DAT" FOR INPUT AS #2
c$ = INPUT$(8, #2)
PRINT #1, "after"; e%
CLOSE #1
END
oops:
e% = ERR
RESUME NEXT
