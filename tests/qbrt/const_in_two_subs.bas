' The same CONST in two SUBs: each is local to its SUB.
DECLARE SUB a ()
DECLARE SUB b ()
a
b
SUB a
  CONST pi = 3.1415926#
  PRINT pi * 2
END SUB
SUB b
  CONST pi = 3.1415926#
  PRINT pi * 3
END SUB
