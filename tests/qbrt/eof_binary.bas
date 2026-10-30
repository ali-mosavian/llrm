' EOF(n) on a BINARY file crashed at m16: B$FLOF stored a 32-bit value into a 16-bit spill slot, over the saved BP
' (the old instruction selector), and B$FEOF returned to 0000:0000.
OPEN "IB.DAT" FOR OUTPUT AS #2
PRINT #2, "hello";
CLOSE #2
OPEN "IB.DAT" FOR BINARY AS #2
PRINT EOF(2)
a$ = INPUT$(5, #2)
PRINT EOF(2); a$
CLOSE #2
KILL "IB.DAT"
