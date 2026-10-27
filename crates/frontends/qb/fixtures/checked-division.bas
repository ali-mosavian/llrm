1 REM BC /E /X: x 1.#INF, a-2147483648, h 11 42, i-32768, h 11 46, m-32768, after
10 DIM z AS SINGLE, x AS SINGLE, a AS LONG, b AS LONG, i AS INTEGER, j AS INTEGER
15 ON ERROR GOTO 100
20 x = 1 / z
22 PRINT "x"; x
30 a = -2147483647 - 1: b = -1
32 a = a \ b
34 PRINT "a"; a
40 i = -32768: j = -1
42 i = i \ j
44 PRINT "i"; i
46 i = 7 MOD j + 7 MOD (j + 1)
48 PRINT "m"; i
50 PRINT "after"
60 END
100 PRINT "h"; ERR; ERL
110 RESUME NEXT
