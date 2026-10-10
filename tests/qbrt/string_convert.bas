' ASC, HEX$, OCT$, MKx$, CVx, MID$ as a statement and fixed-length strings in a TYPE: the runtime
' had none of these entries, so a program using them did not link.
TYPE T
  n AS INTEGER
  s AS STRING * 3
END TYPE
DIM r AS T
a$ = "Hello"
PRINT ASC(a$); ASC("z")
PRINT HEX$(255); HEX$(-1); HEX$(0); HEX$(65535); HEX$(70000&); HEX$(-70000)
PRINT OCT$(8); OCT$(-1); OCT$(0); OCT$(100000&)
PRINT LEN(MKI$(1)); LEN(MKL$(1)); LEN(MKS$(1)); LEN(MKD$(1))
PRINT CVI(MKI$(-1234)); CVL(MKL$(123456789)); CVS(MKS$(1.5)); CVD(MKD$(-2.25#))
PRINT ASC(MKI$(258)); ASC(MID$(MKI$(258), 2, 1))
MID$(a$, 2, 2) = "EYXX"
PRINT a$
MID$(a$, 4) = "!"
PRINT a$
MID$(a$, 1, 1) = ""
PRINT a$
b$ = "abc"
MID$(b$, 2) = b$
PRINT b$
r.n = 7
r.s = "ab"
PRINT r.n; "["; r.s; "]"
r.s = "abcdef"
PRINT "["; r.s; "]"
MID$(r.s, 2, 1) = "Z"
PRINT "["; r.s; "]"
PRINT LEN(r.s)
