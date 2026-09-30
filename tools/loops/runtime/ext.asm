; The loop corpus's externals for C and Nib: the report stream (one signed
; decimal per line) and the opaque calls of spec.OPAQUE.
.model medium
.386

.data
ticks   dw 0
digits  db 12 dup (?)

.code
; void report(long v)
public _report
_report proc far
    push bp
    mov bp, sp
    push si
    push di
    mov eax, dword ptr [bp+6]
    mov di, offset digits + 12
    mov byte ptr [di-1], 10
    mov byte ptr [di-2], 13
    sub di, 2
    xor cx, cx
    test eax, eax
    jns positive
    neg eax
    inc cx
positive:
    mov ebx, 10
more:
    xor edx, edx
    div ebx
    add dl, '0'
    dec di
    mov [di], dl
    test eax, eax
    jnz more
    jcxz unsigned
    dec di
    mov byte ptr [di], '-'
unsigned:
    mov dx, di
    mov cx, offset digits + 12
    sub cx, di
    mov bx, 1
    mov ah, 40h
    int 21h
    pop di
    pop si
    pop bp
    ret
_report endp

; int keep(int x)
public _keep
_keep proc far
    push bp
    mov bp, sp
    mov ax, [bp+6]
    pop bp
    ret
_keep endp

; long keep32(long x)
public _keep32
_keep32 proc far
    push bp
    mov bp, sp
    mov ax, [bp+6]
    mov dx, [bp+8]
    pop bp
    ret
_keep32 endp

public _touch
_touch proc far
    ret
_touch endp

public _tick
_tick proc far
    inc word ptr ticks
    ret
_tick endp

; unsigned tick_count(void): the count since the last call
public _tick_count
_tick_count proc far
    xor ax, ax
    xchg ax, word ptr ticks
    ret
_tick_count endp

; void lcopy(void far *d, void far *s, unsigned n)
public _lcopy
_lcopy proc far
    push bp
    mov bp, sp
    push si
    push di
    push ds
    push es
    les di, [bp+6]
    lds si, [bp+10]
    mov cx, [bp+14]
    cld
    rep movsb
    pop es
    pop ds
    pop di
    pop si
    pop bp
    ret
_lcopy endp

end
