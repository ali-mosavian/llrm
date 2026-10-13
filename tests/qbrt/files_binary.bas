' OPEN FOR BINARY, LOF, GET and PUT into a string, CLOSE, and OPEN again.
f$ = "QBRTBIN.DAT"
OPEN f$ FOR BINARY AS #1
buf$ = "hello, world"
PUT #1, , buf$
PRINT LOF(1)
CLOSE #1
OPEN f$ FOR BINARY AS #2
back$ = SPACE$(5)
GET #2, , back$
PRINT back$; LOF(2)
CLOSE
