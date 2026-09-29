' The STRING-result callee, apart: llrm-qb refuses it for QB 4.5 and PDS 7.1.
'$include: 'CC.BI'

function rStr$ (h as integer)
    trash
    putI 65, h
    rStr$ = "world"
end function
