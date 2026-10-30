1 REM BC /E /X /D: h 6 20, la 2147483647, h 6 22, ls-2147483647, lm 0, h 6 26, in-32768, h 6 28, is 32767, h 6 30, ln-2147483648, ci-32768, h 6 34, cl-32768, fl-2147483648, h 6 38, mix 0, after
10 DIM a AS LONG, b AS LONG, i AS INTEGER, j AS INTEGER, x AS SINGLE
15 ON ERROR GOTO 100
20 a = 2147483647: a = a + 1: PRINT "la"; a
22 a = -2147483647: a = a - 2: PRINT "ls"; a
24 a = 65536: a = a * a: PRINT "lm"; a
26 i = -32768: i = -i: PRINT "in"; i
28 i = -32768: i = i - 1: PRINT "is"; i
30 a = -2147483647 - 1: a = -a: PRINT "ln"; a
32 x = 40000: i = CINT(x): PRINT "ci"; i
34 a = 40000: i = CINT(a): PRINT "cl"; i
36 x = 3E+09: a = x: PRINT "fl"; a
38 i = 300: j = i * 200 \ 200: PRINT "mix"; j
40 PRINT "after"
50 END
100 PRINT "h"; ERR; ERL
110 RESUME NEXT
