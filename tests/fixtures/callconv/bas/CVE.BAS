' BYVAL and SEG in a SUB definition: PDS 7.1 and VBDOS.
'$include: 'CC.BI'

sub pByv (byval h as integer, byval v as long, byval s as single, byval d as double, byval t as integer)
    trash
    putI 86, h: putL 87, v: putF 88, s: putD 89, d: putI 93, t
    h = 0: v = 0
end sub

' Never called: BC's callers of the SEG DECLARE push a far pointer, while BC
' compiles this definition with a near one (retf 6). No reference to hold to.
sub pSegd (h as integer, seg v as integer, t as integer)
    trash
    putI 94, h: putI 95, v: putI 96, t
    v = v + 1
end sub
