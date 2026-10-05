' dialect: vbdos
' Own frames: ON ERROR / RESUME NEXT across procedures, ERR and ERL. The error lands in a
' procedure the module handler serves, through the runtime's frame chain.
DECLARE SUB caller (d AS INTEGER)
DECLARE FUNCTION inner% (d AS INTEGER)
DECLARE FUNCTION leaf% (d AS INTEGER)
DEFINT A-Z
ON ERROR GOTO h
caller 0
caller 2
PRINT "done"; ERR
END
h:
PRINT "err"; ERR; ERL
RESUME NEXT
SUB caller (d AS INTEGER)
DIM a AS LONG, b AS INTEGER
900 a = 100
905 b = inner(d)
908 PRINT "caller"; a; b
END SUB
FUNCTION inner% (d AS INTEGER)
DIM c AS INTEGER
910 c = leaf(d)
920 inner% = c + 1
END FUNCTION
FUNCTION leaf% (d AS INTEGER)
930 IF d = 0 THEN ERROR 11
940 leaf% = 10 \ d
END FUNCTION
