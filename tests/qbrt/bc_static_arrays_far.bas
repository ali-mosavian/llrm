' A static array whose bound is a LONG constant (`a(16000&)`) is allocated in far memory when the program starts, so two
' of them together exceed DGROUP; llrm-qb put them in DGROUP and panicked writing the object (BC: a(30000) as INTEGER stays near).
OPEN "L.TXT" FOR OUTPUT AS #1
DIM SHARED a(16000&) AS INTEGER
DIM SHARED b(16000&) AS INTEGER
FOR i = 0 TO 16000
  a(i) = i: b(i) = -i
NEXT
s& = 0: t& = 0
FOR i = 0 TO 16000
  s& = s& + a(i): t& = t& + b(i)
NEXT
PRINT #1, a(16000); b(16000); s&; t&
PRINT #1, UBOUND(a); LBOUND(b)
CLOSE #1
