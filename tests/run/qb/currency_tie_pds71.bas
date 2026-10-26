' A DOUBLE constant times 10000 is folded at 53 bits where BC rounds at 64: a product within 1e-16 of a .5 tie
' BC prints .0001 and .0003; the compiler prints 0 for the first.
' dialect: pds71
' flags: -O2 --isel=dag
DIM c AS CURRENCY, d AS DOUBLE
d = 0.00005#: c = d: PRINT c
d = 0.00025#: c = d: PRINT c
