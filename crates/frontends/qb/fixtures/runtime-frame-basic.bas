' Adapted to printable QB/VBDOS output from FreeBASIC's
' tests/qb/local-suffixvar-overrides-shared-suffixvar.bas.
declare sub report (tag as string, n as long)

dim n as long
n = 1

call report("FRAME", n)

sub report (tag as string, n as long)
    dim zeroInteger as integer
    dim zeroLong as long

    if zeroInteger = 0 and zeroLong = 0 then
        print "FRAME ZERO"
    else
        print "FRAME DIRTY"
    end if
end sub
