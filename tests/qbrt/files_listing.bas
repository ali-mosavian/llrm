' FILES: the current directory, the entries that match with <DIR> for a directory, and the free space
DEFINT A-Z
ON ERROR GOTO h
MKDIR "FLSTDIR"
FOR i = 1 TO 6
    OPEN "FLST" + LTRIM$(STR$(i)) + ".TMP" FOR OUTPUT AS #1
    CLOSE #1
NEXT
OPEN "FLSTLONG.AB" FOR OUTPUT AS #1
CLOSE #1
FILES "FLST*.*"
FILES "FLST1.TMP"
FILES "NOSUCH*.*"
PRINT "done"
END
h:
PRINT "error"; ERR
RESUME NEXT
