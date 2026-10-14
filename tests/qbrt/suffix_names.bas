' A name declared AS a type is the variable its suffix spells: `v AS LONG` and `v&` are one (BC, in a SUB and across SUBs),
' a name with another suffix is another variable (`w!` beside `w&`).  llrm-qb kept `v&` apart from `v AS LONG`: a shared
' address held in `vbase&` read 0 in the SUB that used it, and demos ported to m32 POKEd the interrupt table.
DECLARE SUB p ()
DECLARE SUB q ()
DIM SHARED v AS LONG
DIM SHARED w&
v& = 5
w = 3
PRINT v&; v; w; w&
p
q
PRINT v&; v; w&
SUB p
  PRINT v&; v
  v& = 7
  w& = 4
END SUB
SUB q
  DIM u AS LONG
  u& = 9
  PRINT u; u&; w&; w
END SUB
