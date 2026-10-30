TYPE Rec
  a AS INTEGER
  s AS STRING * 10
  d AS DOUBLE
END TYPE
CONST c% = 5
DIM SHARED sh%
DIM x%, y%, l&, s!, d#, s$, t$
DIM r AS Rec
DIM arr%(10), arl&(10), ars!(10), ard#(10), sa$(10), m%(5, 5), rr(3) AS Rec
DECLARE SUB mysub (a%, b$)
DECLARE FUNCTION myfn% (a%)
DECLARE SUB arrsub (a%())
lab:
