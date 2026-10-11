' READ of every type from DATA, quoted strings, and RESTORE.
DIM a%(1 TO 3)
DATA 42, -7, 32767
DATA 100000, "quoted, with comma", "second", 3.7, 12
FOR i% = 1 TO 3: READ a%(i%): NEXT
READ b&, q$, r$, rounded%, plain%
PRINT a%(1); a%(2); a%(3); b&
PRINT "["; q$; "]["; r$; "]"
PRINT rounded%; plain%
RESTORE
READ x%
PRINT x%
RESTORE tail
READ t1%, t2%
PRINT t1%; t2%
tail:
DATA 11, 22
