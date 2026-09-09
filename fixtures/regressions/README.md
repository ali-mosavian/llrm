# Focused regression objects

`chain-stack-q-O.obj` is real QuickBASIC 4.5 `/O` output from
`suite/chain.bas`, captured during the stack-argument recovery work. Its
constant-divisor remainder feeds another remainder and is reused later.
Unlike the older chain fixture, this compilation has relocated memory
pushes in the recovered argument sequence. Losing their relocation makes
`CONST2` print 0 instead of 13106.

`nbody-stack-p-g2.obj` is real PDS 7.1 `/G2` output from `suite/nbody.bas`.
Its inner-loop multiply has an outer division's constant argument already
pushed below its own arguments. Recovery must keep that argument separate.
