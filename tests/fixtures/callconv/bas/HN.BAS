' Built by BC alone: runs callAll, then prints each mismatch and a count
' to CC.OUT. Wants are hand-derived bytes, most significant first.
'$include: 'CC.BI'
declare sub checkHex (k as integer, what as string, want as string)
declare sub checkStr (k as integer, what as string, want as string)
dim shared fails as integer
dim k as integer

open "CC.OUT" for output as #1
call armX: badRegs: verifyX 63
callAll

checkHex 1, "pInt h", "1111"
checkHex 2, "pInt v", "EDCB"
checkHex 3, "pInt t", "2222"
checkHex 4, "pInt back", "EDCC"
checkHex 5, "pLng h", "1111"
checkHex 6, "pLng v", "EDCBA987"
checkHex 7, "pLng t", "2222"
checkHex 8, "pLng back", "EDCBA988"
checkHex 9, "pSng h", "1111"
checkHex 10, "pSng v", "3FC00000"
checkHex 11, "pSng t", "2222"
checkHex 12, "pSng back", "40400000"
checkHex 13, "pDbl h", "1111"
checkHex 14, "pDbl v", "C002000000000000"
checkHex 15, "pDbl t", "2222"
checkHex 16, "pDbl back", "C012000000000000"
checkHex 17, "pStr h", "1111"
checkStr 18, "pStr v", "hello"
checkHex 19, "pStr t", "2222"
checkStr 20, "pStr back", "hello!"
checkHex 21, "pFix h", "1111"
checkStr 22, "pFix v", "abcdef"
checkHex 23, "pFix t", "2222"
checkStr 24, "pFix back", "XY    "
checkHex 25, "pRec h", "1111"
checkHex 26, "pRec v.a", "0A0B"
checkHex 27, "pRec v.b", "01020304"
checkHex 28, "pRec v.c", "4009000000000000"
checkStr 29, "pRec v.s", "wxyz"
checkHex 30, "pRec t", "2222"
checkHex 31, "pRec back.a", "0A0C"
checkStr 32, "pRec back.s", "WXYZ"
checkHex 33, "pArrI h", "1111"
checkHex 34, "pArrI lbound", "0002"
checkHex 35, "pArrI ubound", "0005"
checkHex 36, "pArrI v(2)", "000A"
checkHex 37, "pArrI v(3)", "0014"
checkHex 38, "pArrI v(4)", "001E"
checkHex 39, "pArrI v(5)", "0028"
checkHex 40, "pArrI t", "2222"
checkHex 41, "pArrI back", "0063"
checkHex 42, "pArrS h", "1111"
checkHex 43, "pArrS lbound", "0000"
checkHex 44, "pArrS ubound", "0002"
checkStr 45, "pArrS v(0)", "a"
checkStr 46, "pArrS v(1)", "bb"
checkStr 47, "pArrS v(2)", "ccc"
checkHex 48, "pArrS t", "2222"
checkStr 49, "pArrS back", "Q"
checkHex 50, "pMix a", "EDCB"
checkHex 51, "pMix b", "EDCBA987"
checkHex 52, "pMix c", "3FC00000"
checkHex 53, "pMix d", "C002000000000000"
checkStr 54, "pMix e", "mix"
checkHex 55, "pMix f", "2222"
checkHex 56, "sVoid", "5555"
checkHex 57, "rInt h", "3333"
checkHex 58, "rInt", "EDCB"
checkHex 59, "rLng h", "3333"
checkHex 60, "rLng", "EDCBA987"
checkHex 61, "rSng h", "3333"
checkHex 62, "rSng", "3FC00000"
checkHex 63, "rDbl h", "3333"
checkHex 64, "rDbl", "C002000000000000"
checkHex 65, "rStr h", "3333"
checkStr 66, "rStr", "world"
' recVal's stack from SP+4 up: d, c, b, a.
checkHex 90, "recVal stack", "EDCBEDCBA9873FC00000C002000000000000"
' recSeg: b.seg-ds, a.seg-ds, *b, *a.
checkHex 91, "recSeg", "00000000C002000000000000EDCB"
checkHex 67, "recSeg back a", "EDCC"
' recCdl's stack from SP+4 up: a, b, c.
checkHex 92, "recCdl stack", "C002000000000000EDCBA987EDCB"
checkHex 68, "aInt", "EDCB"
checkHex 69, "aLng", "EDCBA987"
checkHex 70, "aSng", "3FC00000"
checkHex 71, "aDbl", "C002000000000000"
checkStr 72, "aStr", "abc"
checkHex 73, "xInt", "EDCB"
checkHex 74, "xLng", "EDCBA987"
checkHex 75, "xSng", "3FC00000"
checkHex 76, "xDbl", "C002000000000000"
'$include: 'CURCHK.BI'
'$include: 'V71CHK.BI'

if probeMask%(63) <> &H23 then
    fails = fails + 1
    print #1, "FAIL instrument: verify saw "; hex$(probeMask%(63)); " for a callee losing SI DI DF, want 23"
end if
for k = 1 to 62
    if probeMask%(k) <> 0 then
        fails = fails + 1
        print #1, "FAIL regs case"; k; "lost (1 SI 2 DI 4 BP 8 SP 16 DS 32 DF 64 SS) "; hex$(probeMask%(k))
    end if
next
print #1, "failures"; fails
close #1
end

' Slot k's first len(want) / 2 bytes, most significant first.
sub checkHex (k as integer, what as string, want as string)
    dim i as integer, got as string
    got = ""
    for i = len(want) \ 2 - 1 to 0 step -1
        got = got + right$("0" + hex$(getB%(k, i)), 2)
    next
    if got <> want then
        fails = fails + 1
        print #1, "FAIL "; what; " got "; got; " want "; want
    end if
end sub

' Slot k as a string: length word, then the characters.
sub checkStr (k as integer, what as string, want as string)
    dim i as integer, got as string
    got = ""
    for i = 1 to getB%(k, 0) + 256 * getB%(k, 1)
        if i > 30 then exit for
        got = got + chr$(getB%(k, i + 1))
    next
    if got <> want then
        fails = fails + 1
        print #1, "FAIL "; what; " got '"; got; "' want '"; want; "'"
    end if
end sub
