' A FUNCTION that returns a string: the result is copied to a temporary when
' the frame goes (B$SCPF).
DECLARE FUNCTION Twice$ (s$)
DECLARE FUNCTION Shout$ (s$)
PRINT Twice$("ab"); Shout$("hey")
t$ = Twice$(Shout$("x"))
PRINT t$; LEN(t$)

FUNCTION Twice$ (s$)
    Twice$ = s$ + s$
END FUNCTION

FUNCTION Shout$ (s$)
    Shout$ = UCASE$(s$) + "!"
END FUNCTION
