' RESUME retries the statement that failed.
ON ERROR GOTO repair
n% = 0
v% = 10 \ n%
PRINT "v ="; v%; "tries"; tries%
END

repair:
tries% = tries% + 1
n% = 2
RESUME
