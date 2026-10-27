' dialect: qb45
DECLARE SUB deep (n%)
DEFINT A-Z
x = 5
CLEAR , , 4096
PRINT STR$(x)
deep 10
PRINT "ok"
END
SUB deep (n)
  IF n > 0 THEN deep n - 1
END SUB
