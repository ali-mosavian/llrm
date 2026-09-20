.model medium
.386

extrn _parity_scalar:far

.data
value dd ?
filename db 'VALUE.BIN', 0
stack_space db 1024 dup (?)
stack_top label byte

.code
start:
    mov ax, @data
    mov ds, ax
    cli
    mov ss, ax
    mov sp, offset stack_top
    sti
    fninit
    call far ptr _parity_scalar
    mov word ptr value, ax
    mov word ptr value+2, dx

    mov ah, 3ch
    xor cx, cx
    lea dx, filename
    int 21h
    jc failed
    mov bx, ax
    mov ah, 40h
    mov cx, 4
    lea dx, value
    int 21h
    jc failed
    xor al, al
    jmp finished

failed:
    mov al, 1
finished:
    mov ah, 4ch
    int 21h
end start
