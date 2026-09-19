defint a-z

const LSNEUTRAL = 120

declare function lsScaleByte (byval raw as integer, byval sval as integer) as integer
declare function quakeLightDemo () as long

print "RESULT="; quakeLightDemo()
print "DONE"
end

' From qb-qrender d_surf.bas and qcport render/ls.c.
function lsScaleByte (byval raw as integer, byval sval as integer) as integer
    dim value as long

    value = clng(raw) * sval \ LSNEUTRAL
    if value > 255 then value = 255
    if value < 0 then value = 0
    lsScaleByte = value
end function

function quakeLightDemo () as long
    quakeLightDemo = clng(lsScaleByte(200, 120)) * 1000000 + _
                     clng(lsScaleByte(200, 60)) * 1000 + _
                     lsScaleByte(200, 240) + lsScaleByte(-20, 120)
end function
