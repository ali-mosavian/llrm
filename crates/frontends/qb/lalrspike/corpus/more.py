"""More whole programs, appended after the groups of programs.py (their ids stay): SELECT CASE, metacommands, and the
forms a line can take.
"""



def register(add):

    # ---- SELECT CASE -------------------------------------------------------------------------------------------------
    SUBJECTS = ["x%", "s$", "d#", "l&", "x% + 1", "LEN(s$)", "1", "\"a\"", "x%(1)", "r.a"]
    CASES = [
        "CASE 1", "CASE 1, 2, 3", "CASE 1 TO 5", "CASE IS > 5", "CASE IS = x%", "CASE IS <> 3", "CASE IS < 2, IS > 8", "CASE \"a\"", "CASE \"a\", \"b\"", "CASE \"a\" TO \"z\"", "CASE IS > \"a\"",
        "CASE x%", "CASE x% + 1", "CASE 1.5", "CASE 70000", "CASE -1", "CASE 5 TO 1", "CASE 1 TO", "CASE TO 5", "CASE", "CASE IS", "CASE IS 5", "CASE 1; 2", "CASE 1 THEN", "CASE 1 TO 2 TO 3",
        "CASE 1 TO 5, 7, IS > 9", "CASE s$", "CASE s$ + \"a\"", "CASE LEN(s$)", "CASE 1 TO x%", "CASE x% TO 9", "CASE IS >= 1 AND 2", "CASE NOT 1", "CASE (1)", "CASE 1: y% = 1", "CASE 1 : y% = 1",
        "CASE ELSE", "CASE ELSE: y% = 1", "CASE ELSE, 1", "CASE IS = IS", "CASE 1,", "CASE ,1", "CASE 1 2", "CASE &H10", "CASE 1E3", "CASE 1D3", "CASE r.a", "CASE FNf%(1)",
    ]
    for subject in SUBJECTS:
        for case in CASES:
            add("select", f"{subject!r} {case!r}", "TYPE R", "a AS INTEGER", "END TYPE", "DIM x%, y%, s$, d#, l&, x%(3), r AS R", "DEF FNf%(z%) = z%", f"SELECT CASE {subject}", case, "y% = 2", "END SELECT")
    for body in ["SELECT CASE x%", "SELECT CASE x%\nEND SELECT", "SELECT CASE x%\nCASE 1\nCASE 2\nEND SELECT", "SELECT CASE x%\nCASE ELSE\nEND SELECT", "SELECT CASE x%\nCASE ELSE\nCASE ELSE\nEND SELECT",
                 "SELECT CASE x%\nCASE 1\nCASE ELSE\nCASE 2\nEND SELECT", "SELECT CASE x%\nCASE 1\nSELECT CASE y%\nCASE 2\nEND SELECT\nEND SELECT", "SELECT CASE x%\nCASE 1\nSELECT CASE y%\nCASE 2\nEND SELECT",
                 "SELECT x%\nCASE 1\nEND SELECT", "SELECT CASE\nCASE 1\nEND SELECT", "SELECT CASE x%, y%\nCASE 1\nEND SELECT", "SELECT CASE x% CASE 1\nEND SELECT", "SELECT CASE x%: CASE 1: y% = 1: END SELECT",
                 "SELECT CASE x%\nCASE 1: y% = 1\nCASE 2: y% = 2\nEND SELECT", "SELECT CASE x%\ny% = 1\nCASE 1\nEND SELECT", "SELECT CASE x%\nCASE 1\nEXIT SELECT\nEND SELECT", "SELECT CASE x%\nCASE 1\nEXIT DO\nEND SELECT",
                 "DO\nSELECT CASE x%\nCASE 1\nEXIT DO\nEND SELECT\nLOOP", "FOR x% = 1 TO 2\nSELECT CASE x%\nCASE 1\nEXIT FOR\nEND SELECT\nNEXT", "SELECT CASE x%\nCASE 1\nGOTO a\nEND SELECT\na:", "a:\nSELECT CASE x%\nCASE 1\nGOTO a\nEND SELECT",
                 "SELECT CASE x%\nCASE 1\nIF y% THEN\nEND IF\nEND SELECT", "IF y% THEN\nSELECT CASE x%\nCASE 1\nEND IF\nEND SELECT", "SELECT CASE x%\nCASE 1\nFOR i% = 1 TO 2\nCASE 2\nNEXT\nEND SELECT",
                 "SELECT CASE x%\nCASE 1\nDIM z%\nEND SELECT", "SELECT CASE x%\nCASE 1\nDATA 1\nEND SELECT", "SELECT CASE x%\nCASE 1\nSUB q\nEND SELECT", "SELECT CASE x%\nCASE 1\nEND\nEND SELECT",
                 "SELECT CASE x%\nCASE 1\nRETURN\nEND SELECT", "SELECT CASE x%\nCASE 1\nREM c\n' c\nCASE 2\nEND SELECT", "SELECT CASE x%\n' c\nCASE 1\nEND SELECT", "SELECT CASE x% ' c\nCASE 1 ' c\nEND SELECT ' c",
                 "SELECT CASE x%\nCASE 1\n10 PRINT\nEND SELECT", "SELECT CASE x%\n10 CASE 1\nEND SELECT", "10 SELECT CASE x%\nCASE 1\nEND SELECT", "SELECT CASE x%\nCASE 1\nEND SELECT\nEND SELECT", "END SELECT",
                 "SELECT CASE x%\nCASE 1\nEND  SELECT", "SELECT CASE x%\nCASE 1\nENDSELECT", "SELECT CASE x%\nCASE 1\nEND\nSELECT", "select case x%\ncase 1\nend select", "Select Case x%\nCase Else\nEnd Select"]:
        add("select", f"structure {body!r}", "DIM x%, y%, i%", *body.split("\n"))

    # ---- metacommands -----------------------------------------------------------------------------------------------
    for line in ["'$STATIC", "'$DYNAMIC", "REM $STATIC", "REM $DYNAMIC", "' $DYNAMIC", "'$ DYNAMIC", "'$dynamic", "'$Dynamic", "'$DYNAMIC extra", "'$DYNAMIC: PRINT", "'$FOO", "'$", "'$INCLUDE", "'$INCLUDE:", "'$INCLUDE: ''", "'$INCLUDE: 'nothere.bi'", "'$INCLUDE: nothere.bi",
                 "'$LINES", "'$LINESIZE", "'$FORM", "'$FORM frm", "'$FORM frm.frm", "'$CDECL", "'$DEBUG", "'$NODEBUG", "'$PAGE", "'$TITLE: 'x'", "'$SUBTITLE: 'x'", "$DYNAMIC", "$STATIC", "REM$DYNAMIC", "'$DYNAMIC\t", "  '$DYNAMIC", "x% = 1 '$DYNAMIC", "x% = 1: '$DYNAMIC", "REM '$DYNAMIC"]:
        add("meta", f"line {line!r}", "DIM x%", line, "DIM a(10)", "a(1) = 1")
        add("meta", f"line {line!r} after dim", "DIM x%", "DIM a(10)", line, "REDIM a(20)", "a(1) = 1")
        add("meta", f"line {line!r} in sub", "SUB p", line, "DIM a(10)", "END SUB")
    for first, second in [("'$STATIC", "'$DYNAMIC"), ("'$DYNAMIC", "'$STATIC"), ("'$DYNAMIC", "'$DYNAMIC")]:
        add("meta", f"{first} then {second}", first, "DIM a(10)", second, "DIM b(10)", "REDIM a(20)", "ERASE b")
    add("meta", "dynamic then redim small", "'$DYNAMIC", "DIM a(100)", "REDIM a(200)", "ERASE a", "REDIM a(5, 5)")
    add("meta", "static then redim", "'$STATIC", "DIM a(100)", "REDIM a(200)")
    add("meta", "dynamic dim const bound", "'$DYNAMIC", "CONST n = 100", "DIM a(n)", "a(1) = 1")
    add("meta", "dynamic big", "'$DYNAMIC", "DIM a(32767)", "a(1) = 1")

    # ---- the forms a line can take --------------------------------------------------------------------------------------
    LINES = [
        "x% = 1", "x% = 1: y% = 2", "x% = 1 : y% = 2", ": x% = 1", "x% = 1 :", "x% = 1 ::", ":: x% = 1", "x% = 1 : : y% = 2", ":", "::", "  ", "\t", "\tx% = 1", "x%\t=\t1", "x%=1", "x% =1", "x%= 1", "X% = 1", "x % = 1", "x %= 1", "x% = 1 ' c", "x% = 1 REM c", "x% = 1: REM c", "x% = 1 :REM c",
        "REM", "REM c", "REMc", "REM: x% = 1", "' c: x% = 1", "'", "'c", "'''", "x% = 1 _", "x% = _\n1", "x% = 1 + _\n2", "x% _\n= 1", "x_\n% = 1", "PRINT _\n1", "PRINT \"a_\nb\"", "PRINT \"a\" _\n; \"b\"", "' c _\nx% = 1", "REM c _\nx% = 1", "x% = 1 _ ' c\ny% = 2", "_", "_\nx% = 1", "x% = 1\n_", "x% = 1 _\n\ny% = 2", "x% = _ \n1",
        "10 x% = 1", "010 x% = 1", "10x% = 1", "10: x% = 1", "10 : x% = 1", "10\nx% = 1", "10 10 x% = 1", "0 x% = 1", "65535 x% = 1", "65536 x% = 1", "-1 x% = 1", "1.5 x% = 1", "1E3 x% = 1", "&H10 x% = 1", "10 'c", "10 REM c", "10 ' c\n10 x% = 1", "20 x% = 1\n10 y% = 2", "10 x% = 1\n10 y% = 2",
        "a: x% = 1", "a:x% = 1", "a : x% = 1", "a:\nx% = 1", "a: a: x% = 1", "a: b: x% = 1", "A: x% = 1\na: y% = 2", "a1: x% = 1", "a.b: x% = 1", "a_b: x% = 1", "a$: x% = 1", "a%: x% = 1", "_a: x% = 1", "1a: x% = 1", "ab12345678901234567890123456789012345678901234567890: x% = 1",
        "GOTO a\na: x% = 1", "GO TO a\na: x% = 1", "GoTo a\na:", "GOTOa\na:", "GOSUB a\nEND\na: RETURN", "GO SUB a\nEND\na: RETURN", "ON x% GOTO a, b\na:\nb:", "ON x% GO TO a\na:", "ON x% GOTO a,\na:", "ON x% GOTO ,a\na:", "ON x% GOTO\na:", "ON x% GOTO a b\na:\nb:",
        "IF x% THEN y% = 1", "IF x% THEN y% = 1 ELSE y% = 2", "IF x% THEN\ny% = 1\nEND IF", "IF x% THEN\ny% = 1\nENDIF", "IF x% THEN\ny% = 1\nEND  IF", "IF x% THEN\ny% = 1\nELSE IF y% THEN\ny% = 2\nEND IF\nEND IF", "IF x% THEN\ny% = 1\nELSEIF y% THEN\ny% = 2\nEND IF", "IF x% THEN y% = 1\nELSE y% = 2", "IF x% THEN: y% = 1", "IF x% THEN : y% = 1", "IF x% THEN\n: y% = 1\nEND IF",
        "IF x% THEN 10\n10 y% = 1", "IF x% GOTO 10\n10 y% = 1", "IF x% THEN GOTO 10\n10 y% = 1", "IF x% THEN y% = 1 ELSE 10\n10 y% = 1", "IF x% THEN 10 ELSE 20\n10 y% = 1\n20 y% = 2", "IF x% THEN 10 ELSE y% = 2\n10 y% = 1", "IF x% THEN y% = 1 ELSE GOTO 10\n10 y% = 1", "IF x% THEN ELSE y% = 1", "IF x% THEN y% = 1 ELSE", "IF x% THEN y% = 1 ELSE ELSE y% = 2", "IF x% THEN IF y% THEN i% = 1 ELSE i% = 2 ELSE i% = 3",
        "IF x% THEN IF y% THEN i% = 1 ELSE i% = 2", "IF x% = 1 THEN y% = 1: i% = 2 ELSE y% = 3: i% = 4", "IF x% THEN y% = 1: ELSE y% = 2", "IF x% THEN y% = 1 : ELSE y% = 2", "IF (x%) THEN y% = 1", "IF x% THEN (y%) = 1", "IF x% THEN\nELSE\nEND IF", "IF x% THEN\nELSEIF y% THEN\nELSE\nEND IF", "IF THEN y% = 1", "IF x% y% = 1", "IF x% THEN THEN y% = 1",
        "WHILE x%\nWEND", "WHILE x%: WEND", "WHILE x% : y% = 1 : WEND", "WHILE\nWEND", "WHILE x%\nEND WHILE", "DO\nLOOP", "DO: LOOP", "DO WHILE x%\nLOOP", "DO UNTIL x%\nLOOP", "DO\nLOOP WHILE x%", "DO\nLOOP UNTIL x%", "DO WHILE x%\nLOOP UNTIL y%", "DO\nLOOP WHILE", "DO WHILE\nLOOP", "DO x%\nLOOP", "LOOP", "DO\nWEND",
        "FOR i% = 1 TO 2\nNEXT", "FOR i% = 1 TO 2: NEXT", "FOR i% = 1 TO 2\nNEXT i%", "FOR i% = 1 TO 2\nNEXT j%", "FOR i% = 1 TO 2\nNEXT i%, j%", "FOR i% = 1 TO 2\nFOR j% = 1 TO 2\nNEXT j%, i%", "FOR i% = 1 TO 2\nFOR j% = 1 TO 2\nNEXT i%, j%", "FOR i% = 1 TO 2\nFOR j% = 1 TO 2\nNEXT i%\nNEXT j%", "FOR i% = 1 TO 2 STEP 2\nNEXT", "FOR i% = 1 TO 2 STEP\nNEXT", "FOR i% = 1 TO\nNEXT", "FOR i% = 1\nNEXT", "FOR i% TO 2\nNEXT", "FOR = 1 TO 2\nNEXT",
        "FOR i% = 1 TO 2\nNEXT\nNEXT", "NEXT", "FOR i = 1 TO 2\nNEXT i%", "FOR i% = 1 TO 2\nNEXT i", "FOR i$ = 1 TO 2\nNEXT", "FOR a(1) = 1 TO 2\nNEXT", "FOR i% = 1 TO 2 STEP 0\nNEXT", "FOR i% = 1.5 TO 2.5\nNEXT", "FOR i% = \"a\" TO 2\nNEXT", "FOR i% = 1 TO 2 STEP -1\nNEXT", "FOR i# = 1 TO 2\nNEXT i#", "FOR i& = 1 TO 2\nNEXT i&",
        "END", "END IF", "END x%", "STOP", "SYSTEM", "SYSTEM x%", "END: y% = 1", "END\ny% = 1", "RETURN", "RETURN a\na:", "RETURN 10\n10 x% = 1", "EXIT", "EXIT SUB", "EXIT FUNCTION", "EXIT DO", "EXIT FOR", "EXIT WHILE", "EXIT SELECT", "EXIT DEF", "EXIT x%",
        "x% = 1 REM c\nREM d", "PRINT", "PRINT;", "PRINT ;", "PRINT,", "PRINT ,", "PRINT ;;", "PRINT 1;", "PRINT 1,", "PRINT 1;;2", "PRINT 1,,2", "PRINT 1 2", "PRINT 1 \"a\"", "PRINT \"a\" 1", "PRINT \"a\" \"b\"", "PRINT x% y%", "PRINT x%(1) y%", "PRINT (1) (2)", "PRINT (1)(2)", "PRINT 1 (2)", "PRINT TAB(5) 1", "PRINT TAB(5);", "PRINT SPC(5), 1", "PRINT TAB(5) TAB(6)", "PRINT TAB 5", "PRINT TAB()", "PRINT TAB(1, 2)",
        "PRINT USING \"#\"; 1", "PRINT USING \"#\" 1", "PRINT USING \"#\", 1", "PRINT USING; 1", "PRINT USING \"#\";", "PRINT USING x%; 1", "PRINT USING s$; 1; 2", "PRINT USING \"#\"; 1, 2", "PRINT USING \"#\"; 1;", "PRINT USING \"#\"; 1 2", "PRINT #1, USING \"#\"; 1", "PRINT #1,", "PRINT #1", "PRINT #1;", "PRINT #1, 1;", "PRINT #x%, 1", "PRINT # 1, 1", "PRINT #1 , 1", "PRINT #1,1",
        "? 1", "?", "PRINT 1 ? 2", "LPRINT 1", "LPRINT", "LPRINT USING \"#\"; 1", "LPRINT #1, 1", "WRITE", "WRITE 1", "WRITE 1,", "WRITE 1; 2", "WRITE #1, 1", "WRITE #1", "WRITE #1,", "WRITE x%, s$", "WRITE USING \"#\"; 1",
    ]
    for k, body in enumerate(LINES):
        add("forms", f"line {body!r}", "DIM x%, y%, i%, j%, s$", *body.split("\n"))
        add("forms", f"in sub {body!r}", "DIM SHARED x%, y%, i%, j%, s$", "SUB p", *body.split("\n"), "END SUB")
