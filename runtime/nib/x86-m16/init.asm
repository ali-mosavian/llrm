; Nib's start-up hook on real-mode DOS (nib.toml's `init`), which the OS layer's start-up calls
; before `main`: division by zero, and a quotient too wide, fault to INT 0, which the panic handler
; takes until the program exits. Ctrl-C ends the program through INT 23h, which puts the vectors
; back first.
.model medium
.386

extrn N$EDIV:far
extrn LL$SET_VECTOR:far
extrn LL$RESTORE_VECTORS:far

public N$INIT

.code
N$INIT proc far
    push cs
    push offset divide_fault
    push 0
    call far ptr LL$SET_VECTOR
    push cs
    push offset break_handler
    push 23h
    call far ptr LL$SET_VECTOR
    add sp, 12
    retf
N$INIT endp

divide_fault:
    mov ax, DGROUP
    mov ds, ax
    mov es, ax
    call far ptr N$EDIV

; DOS ends the program when this returns by retf with carry set.
break_handler:
    push ds
    push ax
    mov ax, DGROUP
    mov ds, ax
    call far ptr LL$RESTORE_VECTORS
    pop ax
    pop ds
    stc
    retf 2

end
