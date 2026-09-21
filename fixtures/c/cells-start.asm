.model medium
.386
.stack 1024

extrn _fill_cells:far

.data
value dw ?
filename db 'VALUE.BIN', 0

.code
start:
    mov ax, @data
    mov ds, ax
    push 1234
    call far ptr _fill_cells
    add sp, 2
    mov value, ax

    mov ah, 3ch
    xor cx, cx
    lea dx, filename
    int 21h
    jc failed
    mov bx, ax
    mov ah, 40h
    mov cx, 2
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
