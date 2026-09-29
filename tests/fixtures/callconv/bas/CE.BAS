' Callees: each keeps what arrived and writes back through its
' by-reference parameter.
'$include: 'CC.BI'

sub ceStr (k as integer, s as string)
    dim i as integer
    putI k, len(s)
    for i = 1 to len(s)
        putC k, i + 1, asc(mid$(s, i, 1))
    next
end sub

sub pInt (h as integer, v as integer, t as integer)
    trash
    putI 1, h: putI 2, v: putI 3, t
    v = v + 1
end sub

sub pLng (h as integer, v as long, t as integer)
    trash
    putI 5, h: putL 6, v: putI 7, t
    v = v + 1
end sub

sub pSng (h as integer, v as single, t as integer)
    trash
    putI 9, h: putF 10, v: putI 11, t
    v = v * 2
end sub

sub pDbl (h as integer, v as double, t as integer)
    trash
    putI 13, h: putD 14, v: putI 15, t
    v = v * 2
end sub

sub pStr (h as integer, v as string, t as integer)
    trash
    putI 17, h: ceStr 18, v: putI 19, t
    v = v + "!"
end sub

sub pFix (h as integer, v as string, t as integer)
    trash
    putI 21, h: ceStr 22, v: putI 23, t
    v = "XY"
end sub

sub pRec (h as integer, v as Rec, t as integer)
    trash
    putI 25, h: putI 26, v.a: putL 27, v.b: putD 28, v.c: ceStr 29, v.s: putI 30, t
    v.a = v.a + 1
    v.s = "WXYZ"
end sub

sub pArrI (h as integer, v() as integer, t as integer)
    dim i as integer
    trash
    putI 33, h: putI 34, lbound(v): putI 35, ubound(v)
    for i = lbound(v) to ubound(v)
        putI 36 + i - lbound(v), v(i)
    next
    putI 40, t
    v(lbound(v) + 1) = 99
end sub

sub pArrS (h as integer, v() as string, t as integer)
    dim i as integer
    trash
    putI 42, h: putI 43, lbound(v): putI 44, ubound(v)
    for i = lbound(v) to ubound(v)
        ceStr 45 + i - lbound(v), v(i)
    next
    putI 48, t
    v(lbound(v) + 1) = "Q"
end sub

sub pMix (a as integer, b as long, c as single, d as double, e as string, f as integer)
    trash
    putI 50, a: putL 51, b: putF 52, c: putD 53, d: ceStr 54, e: putI 55, f
end sub

sub sVoid
    trash
    putI 56, &H5555
end sub

function rInt% (h as integer)
    trash
    putI 57, h
    rInt% = -&H1235
end function

function rLng& (h as integer)
    trash
    putI 59, h
    rLng& = -&H12345679
end function

function rSng! (h as integer)
    trash
    putI 61, h
    rSng! = 1.5
end function

function rDbl# (h as integer)
    trash
    putI 63, h
    rDbl# = -2.25
end function
