' Requires QBX.LIB and a controlled DOS interrupt contract to execute.
type RegType
    ax as integer
    bx as integer
    cx as integer
    dx as integer
    bp as integer
    si as integer
    di as integer
    flags as integer
end type

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

dim inputRegs as RegType
dim outputRegs as RegType
dim inputRegsX as RegTypeX
dim outputRegsX as RegTypeX

if 0 then
    call interrupt(&H21, inputRegs, outputRegs)
    call interruptx(&H21, inputRegsX, outputRegsX)
end if
print "REFERENCE ONLY pds-interrupt"
end
