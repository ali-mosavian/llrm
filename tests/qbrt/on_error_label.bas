' RESUME to a label of the main code goes there, not to the next statement.
ON ERROR GOTO handler
PRINT "start"
ERROR 9
PRINT "skipped"
after:
PRINT "at after, error"; ERR
END

handler:
PRINT "handler"; ERR
RESUME after
