' CURRENCY callees: PDS 7.1 and VBDOS.
'$include: 'CC.BI'

sub pCur (h as integer, v as currency, t as integer)
    trash
    putI 80, h: putY 81, v: putI 82, t
    v = v * 2
end sub

function rCur@ (h as integer)
    trash
    putI 84, h
    rCur@ = 9876.5432
end function
