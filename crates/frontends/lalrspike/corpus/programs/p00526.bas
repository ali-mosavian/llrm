DIM x%, s$
DECLARE FUNCTION f% (a%, b$)
s$ = f%(x%, s$)
END
FUNCTION f% (a%, b$)
f% = 1
END FUNCTION
