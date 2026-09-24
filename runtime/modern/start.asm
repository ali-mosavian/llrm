.model medium
.386
.dosseg
.stack 512

extrn _start:far
extrn _rt_panic_divide:far
; Under DOSSEG the linker defines these around the uninitialized data.
extrn _edata:byte
extrn _end:byte

public _rt_restore_vectors

.data
; The divide fault's vector before the program took it.
old_divide dd 0

.code
start:
    mov ax, DGROUP
    mov ds, ax
    mov es, ax
    ; C statics without an initializer are in _BSS, which the EXE does not
    ; store: they hold whatever the last program left there until zeroed.
    mov di, offset DGROUP:_edata
    mov cx, offset DGROUP:_end
    sub cx, di
    xor al, al
    cld
    rep stosb
    ; Division by zero, and a quotient too wide, fault to INT 0: the panic
    ; handler takes it until the program exits.
    mov ax, 3500h
    int 21h
    mov word ptr old_divide, bx
    mov word ptr old_divide+2, es
    push ds
    push cs
    pop ds
    mov dx, offset divide_fault
    mov ax, 2500h
    int 21h
    pop ds
    push ds
    pop es
    call far ptr _start
    push ax
    call far ptr _rt_restore_vectors
    pop ax
    mov ah, 4ch
    int 21h

divide_fault:
    mov ax, DGROUP
    mov ds, ax
    mov es, ax
    call far ptr _rt_panic_divide

; Gives INT 0 back to DOS; every exit path calls it.
_rt_restore_vectors proc far
    push ds
    mov ax, DGROUP
    mov ds, ax
    lds dx, old_divide
    mov ax, 2500h
    int 21h
    pop ds
    retf
_rt_restore_vectors endp

end start
