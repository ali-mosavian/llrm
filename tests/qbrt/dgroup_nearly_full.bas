' The program's own data nearly fills DGROUP (a 59,800-byte array: about 1.3 KB of the 64 KB group is left for strings and the
' heap, as with BCOM45's runtime), and it still builds a string, writes a file and reads a line from it. The llrm runtime kept 1.8 KB
' more of its own in DGROUP (a kilobyte of line buffer, the error texts): the link failed "DGROUP exceeds 64k by 240 bytes", and
' DEEDSAKS.BAS linked but ran out of memory after its first frame.
DIM SHARED big%(0 TO 29900)
big%(0) = 1: big%(29900) = 2
a$ = STRING$(300, "x")
PRINT LEN(a$); big%(0) + big%(29900)
OPEN "DGFULL.TXT" FOR OUTPUT AS #1
PRINT #1, "hello"
CLOSE #1
OPEN "DGFULL.TXT" FOR INPUT AS #1
LINE INPUT #1, b$
PRINT b$
CLOSE #1
