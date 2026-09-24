; Pascal16 routines, as a QuickBASIC or Turbo Pascal library spells them:
; upper-case names, arguments pushed first to last, and the callee removes
; them with RETF n.
.model medium
.386

extrn CLAMP:far

public SCALE
public CLAMP_ALL
public SPAN

.code
; i16 SCALE(i16 value, i16 numerator, i16 denominator): value * numerator / denominator.
SCALE proc far
    push bp
    mov bp, sp
    mov ax, [bp+10]
    imul word ptr [bp+8]
    idiv word ptr [bp+6]
    pop bp
    retf 6
SCALE endp

; void CLAMP_ALL(i16 far *values, u16 count, i16 low, i16 high): each value
; replaced by the program's exported CLAMP(value, low, high).
CLAMP_ALL proc far
    push bp
    mov bp, sp
    push si
    push di
    les di, [bp+12]
    mov si, [bp+10]
next:
    test si, si
    jz done
    push es
    push word ptr es:[di]
    push word ptr [bp+8]
    push word ptr [bp+6]
    call CLAMP
    pop es
    mov es:[di], ax
    add di, 2
    dec si
    jmp next
done:
    pop di
    pop si
    pop bp
    retf 10
CLAMP_ALL endp

; Range SPAN(i16 far *values, u16 count): the least and greatest of count > 0
; values, returned as the struct's bytes in dx:ax.
SPAN proc far
    push bp
    mov bp, sp
    push di
    les di, [bp+8]
    mov cx, [bp+6]
    mov ax, es:[di]
    mov dx, ax
more:
    mov bx, es:[di]
    cmp bx, ax
    jge not_lower
    mov ax, bx
not_lower:
    cmp bx, dx
    jle not_higher
    mov dx, bx
not_higher:
    add di, 2
    loop more
    pop di
    pop bp
    retf 6
SPAN endp
end
