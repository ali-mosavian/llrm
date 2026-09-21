.model medium
.386

public _rt_write

.code
_rt_write proc far
    push bp
    mov bp, sp
    push bx
    mov dx, [bp+6]
    mov cx, [bp+8]
    mov bx, 1
    mov ah, 40h
    int 21h
    pop bx
    pop bp
    retf
_rt_write endp

end
