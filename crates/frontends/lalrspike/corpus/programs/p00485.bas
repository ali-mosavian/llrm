TYPE RecT
a AS INTEGER
END TYPE
DIM x%, l&, s!, d#, s$, rec AS RecT, arr%(10)
DECLARE SUB f (a%, b$)
x% = f%(x%, s$)
END
FUNCTION f% (a%, b$)
f% = a%
END FUNCTION
