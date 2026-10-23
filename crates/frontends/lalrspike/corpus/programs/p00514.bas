DIM x%, s$
DECLARE FUNCTION f% (a%, b$)
x% = f%(1, "a")
END
FUNCTION f% (a%, b$)
f% = 1
END FUNCTION
