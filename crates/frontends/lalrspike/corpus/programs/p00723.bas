TYPE Inner
q AS INTEGER
END TYPE
TYPE RecT
a AS INTEGER
s AS STRING * 8
i AS Inner
END TYPE
DIM r AS RecT, r2 AS RecT, rr(3) AS RecT
DIM x%, s$
DECLARE SUB p (z AS RecT)
DECLARE SUB q (z() AS RecT)
s$ = r.s + "a"
END
SUB p (z AS RecT)
END SUB
SUB q (z() AS RecT)
END SUB
