option explicit

type RegTypeX
    ax as integer
    bx as integer
    cx as integer
    dx as integer
    bp as integer
    si as integer
    di as integer
    flags as integer
    ds as integer
    es as integer
end type

declare sub interruptx (intnum as integer, inreg as RegTypeX, outreg as RegTypeX)
