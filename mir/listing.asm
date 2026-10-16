.model medium
.386

public _f@3
.data
.code RMW_TEXT
_f@3 proc far
L0_0:
    mov bx, ax
    add word ptr [bx], dx
    retf
_f@3 endp
end
