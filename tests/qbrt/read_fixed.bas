' READ into fixed-length strings: padded with blanks, cut to fit, quoted.
DIM s AS STRING * 5
DIM t AS STRING * 2
DATA ab, xyz, "q r"
READ s, t
PRINT "["; s; "]["; t; "]"
READ s
PRINT "["; s; "]"
