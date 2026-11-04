; x87probe.asm -- the x87 operations the QB runtime's number formatting uses (runtime/qb/i8out.c quick_digits), with every precision
; and rounding control, on whatever FPU the emulator runs them on, no compiler involved: one hex line of each result.
; A .COM: jwasm -bin x87probe.asm.
.model tiny
.386
.387
.code
org 100h
start:
    jmp main

cw      dw 0
res     dt 0
ires    dq 0
tenpow  dt 0
ten     dt 10.0
mags    dt 1.2345678901234567e-3, 9.999999999999999e15, 5.0e-1, 7.77777777777777777e18, 3.141592653589793238e2
nmags   equ 5
thresh  dt 0.5
big     dt 8.999999999999999e18
half    dt 4503599627370497.5
tiny    dt 0.001
hexd    db '0123456789abcdef'
crlf    db 13,10,'$'
tag     db 0,0,0,'$'

emit_byte:                      ; al -> two hex digits
    push ax
    shr al, 4
    call nib
    pop ax
    and al, 15
nib:
    push bx
    movzx bx, al
    mov dl, hexd[bx]
    mov ah, 2
    int 21h
    pop bx
    ret

emit_mem:                       ; si -> cx bytes, high byte first
    add si, cx
e1: dec si
    mov al, [si]
    call emit_byte
    loop e1
    ret

newline:
    mov dx, offset crlf
    mov ah, 9
    int 21h
    ret

label3:                         ; al = '0'..'9' tag char, bl = second char
    mov dl, al
    mov ah, 2
    int 21h
    mov dl, bl
    int 21h
    mov dl, ' '
    int 21h
    ret

setcw:                          ; ax = control word
    mov cw, ax
    fldcw cw
    ret

main:
    ; --- A: 10^s by repeated multiply, 64-bit precision, nearest: s = 0..27
    mov ax, 037fh
    call setcw
    fld1
    mov cx, 28
    mov bh, 0
pa: fld st(0)
    fstp res
    push cx
    mov al, 'A'
    mov bl, '0'
    add bl, bh
    call label3
    mov si, offset res
    mov cx, 10
    call emit_mem
    call newline
    pop cx
    fld ten
    fmulp st(1), st
    inc bh
    cmp bh, 10
    jb  pa1
    mov bh, 0
pa1:loop pa
    fstp st(0)

    ; --- B: magnitude * 10^s, each precision (24/53/64) and rounding (nearest/down/up/trunc)
    mov bp, 0                   ; control word index
pb: mov bx, bp
    shl bx, 1
    mov ax, cwtab[bx]
    call setcw
    mov di, 0                   ; magnitude index
pm: fld1
    mov cx, 13
pt: fld ten
    fmulp st(1), st
    loop pt                     ; st0 = 10^13
    mov bx, di
    imul bx, 10
    fld mags[bx]
    fmulp st(1), st
    fstp res
    mov al, 'B'
    mov bx, bp
    add bl, '0'
    call label3
    mov si, offset res
    mov cx, 10
    call emit_mem
    call newline
    inc di
    cmp di, nmags
    jb  pm
    inc bp
    cmp bp, 12
    jb  pb

    ; --- C: fistp qword of values below 2^63, trunc and nearest and down
    mov ax, 0f7fh
    call setcw
    fld big
    fistp ires
    mov al, 'C'
    mov bl, 't'
    call label3
    mov si, offset ires
    mov cx, 8
    call emit_mem
    call newline
    mov ax, 037fh
    call setcw
    fld half
    fistp ires
    mov al, 'C'
    mov bl, 'n'
    call label3
    mov si, offset ires
    mov cx, 8
    call emit_mem
    call newline

    ; --- D: difference = product - integral - threshold, then its comparison with 0.001 (status word C0-C3)
    mov ax, 037fh
    call setcw
    fld mags
    fld mags+10
    fmulp st(1), st
    fld big
    fsubp st(1), st
    fld thresh
    fsubp st(1), st
    fld st(0)
    fstp res
    mov al, 'D'
    mov bl, 'd'
    call label3
    mov si, offset res
    mov cx, 10
    call emit_mem
    call newline
    fld tiny
    fcomp
    fnstsw ax
    mov word ptr res, ax
    mov al, 'D'
    mov bl, 'c'
    call label3
    mov si, offset res
    mov cx, 2
    call emit_mem
    call newline
    fstp st(0)
    mov ax, 4c00h
    int 21h

cwtab dw 007fh, 047fh, 087fh, 0c7fh, 027fh, 067fh, 0a7fh, 0e7fh, 037fh, 077fh, 0b7fh, 0f7fh   ; 24, 53, 64 bits x nearest, down, up, trunc
end start
