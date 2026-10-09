; QB's far/Pascal entries adapt descriptor offsets to portable regparm3 C.
.model medium
.386

extrn _qb_string_assign@3:near
extrn _qb_string_delete@3:near
extrn _qb_string_space@3:near

public B$SASS
public B$SPAC
public B$STDL

RUNTIME_TEXT segment para public 'CODE'

; The source is pushed before the destination, so destination is closest to
; the far return address.  The portable C boundary is regparm3: source in AX,
; destination in DX.
B$SASS proc far
    push bp
    mov bp, sp
    mov dx, word ptr [bp+6]
    mov ax, word ptr [bp+8]
    call _qb_string_assign@3
    pop bp
    retf 4
B$SASS endp

B$SPAC proc far
    push bp
    mov bp, sp
    mov ax, word ptr [bp+6]
    call _qb_string_space@3
    pop bp
    retf 2
B$SPAC endp

B$STDL proc far
    push bp
    mov bp, sp
    mov ax, word ptr [bp+6]
    call _qb_string_delete@3
    pop bp
    retf 2
B$STDL endp

RUNTIME_TEXT ends
end
