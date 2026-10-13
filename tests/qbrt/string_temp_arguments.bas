' A file name built from a string expression is a temporary the file statements free: OPEN, BSAVE and BLOAD did not, so the 21st
' call raised String formula too complex (the 20 string temporaries were all taken) and the deedlines demo died there after its
' seventh BenchMark's BSAVE "V" + LTRIM$(STR$(phase)) + ".BIN".
DEF SEG = &HB800
FOR i% = 1 TO 30
  BSAVE "ST" + LTRIM$(STR$(i% MOD 3)) + ".BIN", 0, 16
NEXT
PRINT "bsave"
FOR i% = 1 TO 30
  BLOAD "ST" + LTRIM$(STR$(i% MOD 3)) + ".BIN", 0
NEXT
PRINT "bload"
FOR i% = 1 TO 30
  OPEN "ST" + LTRIM$(STR$(i% MOD 3)) + ".BIN" FOR BINARY AS #1
  CLOSE #1
NEXT
PRINT "open"
