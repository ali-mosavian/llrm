' dialect: pds71
' mask: module \w+\s+at address [0-9A-Fa-f]+:[0-9A-Fa-f]+
' known: Microsoft's B$STOP prints "in line No line number" for llrm-qb's module where BC's prints nothing there; cause not found (the module headers differ in bytes 10-13)
PRINT "before"
STOP
PRINT "never"
END
