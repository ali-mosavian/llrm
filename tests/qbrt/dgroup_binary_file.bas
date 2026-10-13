' DGROUP nearly full (a 61,402-byte array: well under a kilobyte left for strings and the heap): OPEN FOR BINARY, GET and PUT need
' only the file's block, as with BCOM45's runtime.  The llrm runtime gave every file a kilobyte of line buffer at OPEN, so the
' deedlines demo died "Out of memory" at its first OPEN FOR BINARY after BSAVE had been through the same heap.
DIM SHARED big%(0 TO 30700)
big%(0) = 1: big%(30700) = 2
a$ = "ab"
OPEN "DGBIN.DAT" FOR BINARY AS #1
PUT #1, 1, a$
a$ = "xy"
GET #1, 1, a$
CLOSE #1
PRINT a$; big%(0) + big%(30700)
