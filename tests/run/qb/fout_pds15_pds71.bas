' PDS 7.1 prints a DOUBLE with 15 digits, rounded half up (QB 4.5 and VBDOS print 16); expected output from PDS 7.1 BC.
' dialect: pds71
DIM b0 AS DOUBLE, x AS DOUBLE
FOR n% = 1 TO 3
  READ b0
  FOR k% = 0 TO 7
    x = b0 + k% / 8#
    PRINT x
  NEXT
NEXT
DATA 999999999999999, 922337203685477, 99999999999999
