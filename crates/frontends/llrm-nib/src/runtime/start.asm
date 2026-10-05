.model medium
.386
; No .dosseg: its order puts every segment outside DGROUP before it, and a far one the image does
; not store would then have its zeros stored. The order here is the linker's, by the first
; appearance of each class: code, DGROUP (data, bss, stack), far data, far bss.
extrn _main:far
extrn N$EDIV:far
extrn BSS_LAST:byte
extrn FBSS_LAST:byte
extrn N$OTOP:word
extrn N$OPSP:word
extrn N$OSLO:word
extrn N$OSIV:far
extrn N$OVEC:far

.code
.data
.data?
; The near uninitialised data runs from here to dos.asm's BSS_LAST, linked last; the far, class
; FAR_BSS, from FBSS_FIRST in this object to dos.asm's FBSS_LAST. The EXE stores neither.
BSS_FIRST label byte
; Nib keeps arrays in the frame: 4 KB, as Open Watcom gives a DOS program.
.stack 4096
FARDATA_ORDER segment para public 'FAR_DATA'
FARDATA_ORDER ends
FBSS_BEG segment para public 'FAR_BSS'
public FBSS_FIRST
FBSS_FIRST label byte
    db 16 dup (?)
FBSS_BEG ends

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
    ; The stack is the last of DGROUP before the heap, so it starts where the near bss ends.
    ; Nothing below the limit but the panic's frames, DOS and an interrupt.
    mov ax, offset DGROUP:BSS_LAST
    add ax, 512
    mov N$OSLO, ax
    ; Statics without an initializer are in _BSS, which the EXE does not
    ; store: they hold whatever the last program left there until zeroed.
    mov di, offset DGROUP:BSS_FIRST
    mov cx, offset DGROUP:BSS_LAST
    sub cx, di
    xor al, al
    cld
    rep stosb
    ; The far uninitialised data, a pass of at most 64K at a time. A label's segment is its frame,
    ; which the linker shares among these segments: its paragraph is the frame's plus the
    ; offset's sixteenths.
    mov bp, bx                     ; the PSP, past the loop's registers
    mov bx, offset FBSS_FIRST
    shr bx, 4
    add bx, seg FBSS_FIRST
    mov dx, offset FBSS_LAST
    shr dx, 4
    add dx, seg FBSS_LAST
far_clear:
    cmp bx, dx
    jae far_cleared
    mov ax, dx
    sub ax, bx
    cmp ax, 1000h
    jbe far_pass
    mov ax, 1000h
far_pass:
    mov es, bx
    add bx, ax
    mov cx, ax
    shl cx, 3                      ; words: eight to a paragraph
    xor di, di
    xor ax, ax
    rep stosw
    jmp far_clear
far_cleared:
    mov bx, bp
    mov ax, DGROUP
    mov es, ax
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
