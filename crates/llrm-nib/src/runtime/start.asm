.model medium
.386
.dosseg
; Nib keeps arrays in the frame: 4 KB, as Open Watcom gives a DOS program.
.stack 4096

extrn _main:far
extrn N$EDIV:far
; Under DOSSEG the linker defines these around the uninitialized data.
extrn _edata:byte
extrn _end:byte
extrn N$OTOP:word
extrn N$OPSP:word
extrn N$OSIV:far
extrn N$OVEC:far

.data
; DOSSEG's _edata and _end mark this segment's class, so it must exist
; even when no program object has uninitialized data.
.data?

.code
start:
    mov bx, es                     ; the PSP, before DS leaves it
    mov ax, DGROUP
    mov ds, ax
    mov es, ax
    ; The stack is data (the machine's stack_is_data): SS is DGROUP and SP
    ; rebased, so a near pointer to a frame cell reaches it through DS.
    mov dx, ss
    sub dx, ax
    shl dx, 4
    cli
    mov ss, ax
    add sp, dx
    sti
    ; Statics without an initializer are in _BSS, which the EXE does not
    ; store: they hold whatever the last program left there until zeroed.
    mov di, offset DGROUP:_edata
    mov cx, offset DGROUP:_end
    sub cx, di
    xor al, al
    cld
    rep stosb
    mov N$OPSP, bx
    ; The near heap starts where the stack ends, the image's last byte in
    ; DGROUP. The program keeps only its image; the heap grows the block.
    mov ax, sp
    mov N$OTOP, ax
    add ax, 15
    shr ax, 4
    mov dx, DGROUP
    sub dx, bx
    add ax, dx
    mov es, bx
    mov bx, ax
    mov ah, 4ah
    int 21h
    push ds
    pop es
    ; Division by zero, and a quotient too wide, fault to INT 0: the panic
    ; handler takes it until the program exits. Ctrl-C ends the program
    ; through INT 23h, which puts the vectors back first.
    push cs
    push offset divide_fault
    push 0
    call far ptr N$OSIV
    push cs
    push offset break_handler
    push 23h
    call far ptr N$OSIV
    add sp, 12
    call far ptr _main
    push ax
    call far ptr N$OVEC
    pop ax
    mov ah, 4ch
    int 21h

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
    call far ptr N$OVEC
    pop ax
    pop ds
    stc
    retf 2

end start
