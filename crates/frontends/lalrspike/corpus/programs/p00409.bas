DIM x%, y%, i%, j%
SELECT CASE x%
CASE 1
y% = 1
CASE ELSE
IF x% THEN
y% = 1
ELSE
y% = 3
END SELECT
END IF
