; DIRTY.COM PROGRAM.EXE: fills all free conventional memory with 0A5h, then runs PROGRAM.EXE.
; DOSBox starts with zeroed RAM and real DOS does not clear what it hands out, so a program that
; reads memory it never wrote (a BSS the start-up forgot to zero) only shows itself on a dirty one.
.model tiny
.386
.code
org 100h
start:
    mov sp, 0FFFEh
    mov bx, 1000h
    mov ah, 4Ah                    ; shrink our block: the COM has all of memory
    int 21h
    mov bx, 0FFFFh
    mov ah, 48h                    ; ask for it all: BX is what is free
    int 21h
    mov ah, 48h
    int 21h
    jc run
    mov es, ax
    mov dx, ax                     ; the block, to free after
    mov bp, bx                     ; paragraphs
fill:
    test bp, bp
    jz filled
    mov ax, bp
    cmp ax, 1000h
    jbe pass
    mov ax, 1000h
pass:
    sub bp, ax
    mov cx, ax
    shl cx, 3                      ; words
    xor di, di
    mov ax, 0A5A5h
    rep stosw
    mov ax, es
    mov bx, 1000h
    add ax, bx
    mov es, ax
    jmp fill
filled:
    mov es, dx
    mov ah, 49h
    int 21h
run:
    push cs
    pop es
    ; the program's pname: the command tail, past the blank, to the CR
    mov si, 82h
    mov di, offset pname
copy:
    lodsb
    cmp al, 13
    je done
    stosb
    jmp copy
done:
    xor al, al
    stosb
    mov ax, cs
    mov word ptr block+4, ax       ; the tail, FCBs at ours
    mov word ptr block+8, ax
    mov word ptr block+12, ax
    mov dx, offset pname
    mov bx, offset block
    mov ax, 4B00h
    int 21h
    jnc exited
    add al, '0'
    mov byte ptr failed+23, al     ; the DOS error, as a digit
    mov dx, offset failed
    mov ah, 9
    int 21h
exited:
    mov ax, 4C00h
    int 21h
failed db 'dirty: exec failed, error 0', 13, 10, '$'
pname db 80 dup (0)
tail db 0, 13
block dw 0, offset tail, 0, 5Ch, 0, 6Ch, 0
end start
