' dialect: pds71
DEFINT A-Z
REDIM a(5)
a(1) = 7
a(5) = 9
REDIM PRESERVE a(10)
PRINT a(1); a(5); a(10)
END
