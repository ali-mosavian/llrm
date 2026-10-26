' ENVIRON adds, replaces and removes a variable (name=value or name value; a lone ; or no value removes) but, as in QB, only fits where the table shrank; a bad spec is error 5
DEFINT A-Z
ON ERROR GOTO h
ENVIRON "PATH="
PRINT "["; ENVIRON$("PATH"); "]"
ENVIRON "QBTST=hello"
PRINT "["; ENVIRON$("QBTST"); "]"
ENVIRON "QBTST=a longer value"
PRINT "["; ENVIRON$("QBTST"); "]"
ENVIRON "QBOTHER   spaced"
PRINT "["; ENVIRON$("QBOTHER"); "]"
ENVIRON "QBTST=short"
PRINT "["; ENVIRON$("QBTST"); "]["; ENVIRON$("QBOTHER"); "]"
ENVIRON "QBTST=;"
PRINT "["; ENVIRON$("QBTST"); "]["; ENVIRON$("QBOTHER"); "]"
ENVIRON "QBOTHER="
PRINT "["; ENVIRON$("QBOTHER"); "]"
ENVIRON "NODELIMITER"
ENVIRON "=x"
ENVIRON ""
PRINT "done"
END
h:
PRINT "error"; ERR
RESUME NEXT
