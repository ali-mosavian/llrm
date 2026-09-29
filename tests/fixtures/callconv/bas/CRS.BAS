' The STRING-array and STRING-result calls, apart: llrm-qb refuses a local
' STRING array for QB 4.5 and PDS 7.1.
'$include: 'CC.BI'

sub callStrs
    dim st as string, ss(0 to 2) as string

    ss(0) = "a": ss(1) = "bb": ss(2) = "ccc"
    call arm: pArrS &H1111, ss(), &H2222: verify 9
    crStr 49, ss(1)
    call arm: st = rStr$(&H3333): verify 24
    crStr 66, st
end sub
