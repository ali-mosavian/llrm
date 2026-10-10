' INPUT's prompt forms: the leading semicolon keeps the cursor, a comma after
' the prompt leaves out the question mark.
CLS
PRINT "start";
INPUT ; k%
PRINT "got"; k%
INPUT "comma", j%
PRINT "got"; j%
INPUT "semi"; i%
PRINT "got"; i%
LOCATE 25, 1
