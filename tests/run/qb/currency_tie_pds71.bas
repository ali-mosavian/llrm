' A DOUBLE constant times 10000 within 1e-16 of a .5 tie: BC multiplies at 64 bits (.0001, .0003), so a fold may not round at 53.
' The optimizer folded 0.00005# * 10000 to 0.5 and printed 0 for the first.
' dialect: pds71
' flags: -O2 --isel=dag
DIM c AS CURRENCY, d AS DOUBLE
d = 0.00005#: c = d: PRINT c
d = 0.00025#: c = d: PRINT c
