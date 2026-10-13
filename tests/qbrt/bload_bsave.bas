' BSAVE and BLOAD: a name with no extension takes .BAS, BLOAD with an offset puts the image there and with none goes where it was
' saved from (text video memory here: an address any target has), and a file that is not a BSAVE image is Bad file mode.
DIM a%(1 TO 4)
FOR i% = 1 TO 4: a%(i%) = i% * 111: NEXT
DEF SEG = VARSEG(a%(1))
BSAVE "BSTEST", VARPTR(a%(1)), 8
FOR i% = 1 TO 4: a%(i%) = 0: NEXT
BLOAD "BSTEST.BAS", VARPTR(a%(2)) - 2
PRINT a%(1); a%(2); a%(3); a%(4)
DEF SEG = &HB800
FOR i% = 0 TO 7: POKE i%, 65 + i%: NEXT
BSAVE "BSVIDEO", 0, 8
FOR i% = 0 TO 7: POKE i%, 32: NEXT
BLOAD "BSVIDEO"
PRINT PEEK(0); PEEK(2); PEEK(6)
OPEN "BSTEST.BAS" FOR BINARY AS #1
PRINT LOF(1)
CLOSE #1
OPEN "BADIMG.TXT" FOR OUTPUT AS #1
PRINT #1, "hello world"
CLOSE #1
ON ERROR GOTO oops
BLOAD "BADIMG.TXT"
PRINT "after"; e%
END
oops:
e% = ERR
RESUME NEXT
