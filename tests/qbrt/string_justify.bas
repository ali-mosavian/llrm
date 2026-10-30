' LSET and RSET on a variable-length string and on a fixed-length one, the source shorter, equal, longer and empty
DEFINT A-Z
TYPE Rec
  f AS STRING * 6
END TYPE
DIM r AS Rec
DIM c AS STRING * 5
a$ = "ABCDEFGH"
LSET a$ = "xy"
PRINT "["; a$; "]"
a$ = "ABCDEFGH"
RSET a$ = "xy"
PRINT "["; a$; "]"
a$ = "ABCD"
LSET a$ = "123456"
PRINT "["; a$; "]"
a$ = "ABCD"
RSET a$ = "123456"
PRINT "["; a$; "]"
a$ = "ABCD"
RSET a$ = LEFT$("wxyz", 4)
PRINT "["; a$; "]"
LSET c = "ab"
PRINT "["; c; "]"
RSET c = "ab"
PRINT "["; c; "]"
RSET c = "abcdefg"
PRINT "["; c; "]"
LSET r.f = "q"
PRINT "["; r.f; "]"
RSET r.f = "q" + "r"
PRINT "["; r.f; "]"
a$ = "ABCD"
LSET a$ = ""
PRINT "["; a$; "]"
END
