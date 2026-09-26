option explicit

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

declare sub interrupt (intnum as integer, inreg as RegType, outreg as RegType)

dim in_registers as RegType
dim out_registers as RegType

in_registers.ax = &H3000
call interrupt(&H21, in_registers, out_registers)

if (out_registers.ax and 255) = 0 then
    print "FAIL interrupt_abi dos_version"
    end
end if

print "PASS interrupt_abi"
