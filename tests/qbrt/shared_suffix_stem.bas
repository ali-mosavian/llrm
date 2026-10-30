' Module k$ and k% share a stem: a SUB's SHARED k% is the integer.
DEFINT A-Z
DECLARE SUB bump ()
k$ = "s"
k% = 5
bump
PRINT k%; k$
SUB bump
  SHARED k%
  k% = k% + 1
END SUB
