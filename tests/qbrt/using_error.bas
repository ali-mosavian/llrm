' A format with no field is an Illegal function call: the text of the format
' is written first, then the message of an error nothing handles, naming the
' module blank padded to eight characters.
PRINT "before"
PRINT USING "A\n\ B"; "xy"
PRINT "not reached"
