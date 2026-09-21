.model medium
.386
.stack 512

extrn _start:far

.code
start:
    mov ax, DGROUP
    mov ds, ax
    call far ptr _start
    mov ah, 4ch
    int 21h

end start
