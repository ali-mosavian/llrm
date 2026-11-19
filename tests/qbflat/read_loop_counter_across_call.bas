' A READ loop over 60 DATA items: the loop counter must survive the call to the runtime's READ. bq32 -O1+ kept it in ECX across
' the call, the runtime used ECX, and the 50th or so READ failed with "Out of DATA".
DIM a(59) AS INTEGER
DATA 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60
FOR i% = 0 TO 59
  READ a(i%)
NEXT
PRINT LTRIM$(STR$(a(59)))
