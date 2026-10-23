DECLARE FUNCTION f% (n%)
PRINT f%(3)
END
FUNCTION f% (n%)
IF n% > 0 THEN f% = n% + f%(n% - 1)
END FUNCTION
