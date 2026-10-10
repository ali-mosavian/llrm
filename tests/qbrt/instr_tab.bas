' INSTR (two and three arguments, empty and missing matches) and PRINT TAB.
a$ = "hello, world, hello"
PRINT INSTR(a$, "hello"); INSTR(a$, "o,"); INSTR(a$, "xyz"); INSTR(a$, "")
PRINT INSTR(8, a$, "hello"); INSTR(20, a$, "h"); INSTR(19, a$, "o"); INSTR(1, "", "a"); INSTR("", "")
PRINT INSTR(3, a$, ""); INSTR(5, "abcabc", "c"); INSTR("aaa", "aa")
PRINT "a"; TAB(5); "b"; TAB(3); "c"
PRINT TAB(10); "x"; TAB(80); "y"
PRINT "col"; TAB(1); "!"
BEEP
SLEEP 1
PRINT "done"
