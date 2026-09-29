' Caller: passes known inputs, keeps what came back. Each call sits between
' arm and verify, which the armed probe uses to check preserved registers.
'$include: 'CC.BI'

sub crStr (k as integer, s as string)
    dim i as integer
    putI k, len(s)
    for i = 1 to len(s)
        putC k, i + 1, asc(mid$(s, i, 1))
    next
end sub

sub callAll
    dim i as integer, l as long, s as single, d as double
    dim st as string, sr as string, fx as string * 6, r as Rec
    dim ai(2 to 5) as integer
    dim segI as integer, segD as double

    i = -&H1235
    call arm: pInt &H1111, i, &H2222: verify 1
    putI 4, i
    l = -&H12345679
    call arm: pLng &H1111, l, &H2222: verify 2
    putL 8, l
    s = 1.5
    call arm: pSng &H1111, s, &H2222: verify 3
    putF 12, s
    d = -2.25
    call arm: pDbl &H1111, d, &H2222: verify 4
    putD 16, d
    st = "hello"
    call arm: pStr &H1111, st, &H2222: verify 5
    crStr 20, st
    fx = "abcdef"
    call arm: pFix &H1111, fx, &H2222: verify 6
    crStr 24, fx
    r.a = &HA0B: r.b = &H1020304: r.c = 3.125: r.s = "wxyz"
    call arm: pRec &H1111, r, &H2222: verify 7
    putI 31, r.a: crStr 32, r.s
    ai(2) = 10: ai(3) = 20: ai(4) = 30: ai(5) = 40
    call arm: pArrI &H1111, ai(), &H2222: verify 8
    putI 41, ai(3)
    i = -&H1235: l = -&H12345679: s = 1.5: d = -2.25: st = "mix"
    call arm: pMix i, l, s, d, st, &H2222: verify 10
    call arm: sVoid: verify 11

    call arm: putI 58, rInt%(&H3333): verify 20
    call arm: putL 60, rLng&(&H3333): verify 21
    call arm: putF 62, rSng!(&H3333): verify 22
    call arm: putD 64, rDbl#(&H3333): verify 23
    callStrs

    call arm: recVal -&H1235, -&H12345679, 1.5, -2.25: verify 30
    segI = -&H1235: segD = -2.25
    call arm: recSeg segI, segD: verify 31
    putI 67, segI
    call arm: recCdl -&H1235, -&H12345679, -2.25: verify 32
    call arm: putI 68, aInt%: verify 33
    call arm: putL 69, aLng&: verify 34
    call arm: putF 70, aSng!: verify 35
    call arm: putD 71, aDbl#: verify 36
    st = "abc": keepDesc st
    call arm: sr = aStr$: verify 37
    crStr 72, sr
    call arm: putI 73, xInt%: verify 38
    call arm: putL 74, xLng&: verify 39
    ' BC reads a CDECL float through SI: no register check.
    putF 75, xSng!
    putD 76, xDbl#

'$include: 'CURCALL.BI'
'$include: 'V71CALL.BI'
end sub
