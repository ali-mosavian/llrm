' LTRIM$ copies into a temporary: LEN(s) stays 3 (an earlier runtime rewrote s in place and printed 0).
s$ = SPACE$(3)
t$ = LTRIM$(s$)
PRINT LEN(s$)
PRINT LEN(t$)
