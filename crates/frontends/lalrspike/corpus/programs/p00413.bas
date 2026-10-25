DIM x%, y%, i%, j%
SELECT CASE x%
CASE 1
y% = 1
CASE ELSE
FOR i% = 1 TO 2
y% = 3
END SELECT
NEXT
