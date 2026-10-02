' bc: /O /E /X /D
' flags: -ftrapv
' INTEGER overflow under BC /D and llrm -ftrapv, where the two agree: the target keeps its old value.
DEFINT A-Z
DIM l AS LONG, m AS LONG, r AS LONG
ON ERROR GOTO h
x = 32767: y = 5
x = 7: x = y + 32767: PRINT "addv"; x
x = 7: x = y - -32768: PRINT "sub"; x
x = 7: x = -32768 - y: PRINT "subv"; x
x = 32767: x = x * 2: PRINT "mul"; x
x = 32767: x = x * y: PRINT "mulv"; x
x = -32768: x = -x: PRINT "neg"; x
l = 2147483647: m = 1: r = 7
r = l + m: PRINT "ladd"; r
r = 7: l = -2147483647 - 1: r = l - m: PRINT "lsub"; r
r = 7: l = 2147483647: m = 2: r = l * m: PRINT "lmul"; r
r = 7: l = -2147483647 - 1: r = -l: PRINT "lneg"; r
x = 7: l = 40000: x = l: PRINT "narrow"; x
x = 7: s! = 40000.5: x = CINT(s!): PRINT "cint"; x
r = 7: s! = 1E10: r = CLNG(s!): PRINT "clng"; r
x = 7: s! = 40000.5: x = s!: PRINT "assign"; x
x = 7: x = -32768: y = -1: x = x \ y: PRINT "idiv"; x
x = 7: x = -32768: y = -1: x = x MOD y: PRINT "mod"; x
END
h:
PRINT "err"; ERR; ERL
RESUME NEXT
