' BC 4.5 takes no BYVAL on a BASIC procedure: arguments pass by reference, the C and Nib ones by value.
defint a-z

const LSNEUTRAL = 120

declare function lsScaleByte% (raw as integer, sval as integer)
declare function BenchQlight& ()

' Read from DATA so no build folds the kernel to a constant.
dim shared inRaw as integer, inLow as integer, inS1 as integer, inS2 as integer, inS3 as integer
read inRaw, inLow, inS1, inS2, inS3
data 200, -20, 120, 60, 240

print ltrim$(str$(BenchQlight&))
end

' From qb-qrender d_surf.bas and qcport render/ls.c.
function lsScaleByte% (raw as integer, sval as integer)
    dim value as long

    value = clng(raw) * sval \ LSNEUTRAL
    if value > 255 then value = 255
    if value < 0 then value = 0
    lsScaleByte% = value
end function

function BenchQlight&
    BenchQlight& = clng(lsScaleByte%(inRaw, inS1)) * 1000000 + _
                     clng(lsScaleByte%(inRaw, inS2)) * 1000 + _
                     lsScaleByte%(inRaw, inS3) + lsScaleByte%(inLow, inS1)
end function
