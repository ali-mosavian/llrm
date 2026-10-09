; QB45 BASIC frame entry and exit, with its measured twenty-byte header.
.model medium
.386

public B$ENRA
public B$EXSA

extrn _qb_string_delete@3:far

.data
qb_current_frame dw 0

QB_TEXT segment para public 'CODE'
assume ds:DGROUP

; CX is the compiler's local-byte count and BX its descriptor count.  The
; measured frame header is twenty bytes; frame descriptors start immediately
; below it and are deleted by B$EXSA.
B$ENRA proc far
    pop ax
    pop dx
    push bp
    mov bp, sp
    push qb_current_frame
    push si
    push di
    push cx
    push bx
    mov bx, ax
    xor ax, ax
    push ax
    push ax
    push ax
    push ax
    push ax
    mov qb_current_frame, bp
    sub sp, cx
    push ds
    pop es
    mov di, sp
    xor ax, ax
    shr cx, 1
    rep stosw
    adc cx, cx
    rep stosb
    push dx
    push bx
    retf
B$ENRA endp

; Keep DX:AX, then remove the runtime header and resume the compiler's epilogue.
B$EXSA proc far
    mov si, sp
    mov bx, ss:[si]
    mov cx, ss:[si+2]
    mov [bp-12], ax
    mov [bp-14], dx
    mov [bp-16], bx
    mov [bp-18], cx
    mov si, bp
    sub si, 20
clear_descriptor:
    cmp word ptr [bp-10], 0
    je short descriptors_cleared
    sub si, 4
    push si
    mov ax, si
    call far ptr _qb_string_delete@3
    pop si
    dec word ptr [bp-10]
    jmp short clear_descriptor
descriptors_cleared:
    mov ax, [bp-2]
    mov qb_current_frame, ax
    mov si, [bp-4]
    mov di, [bp-6]
    mov ax, [bp-12]
    mov dx, [bp-14]
    mov bx, [bp-16]
    mov cx, [bp-18]
    mov sp, bp
    pop bp
    push cx
    push bx
    retf
B$EXSA endp

QB_TEXT ends
end
