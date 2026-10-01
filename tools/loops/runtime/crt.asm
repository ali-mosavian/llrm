; C's start-up for the loop corpus: a 16K stack in DGROUP (C's near
; pointers to locals need SS = DS), then _main. The small STACK segment is
; only for the instructions before the switch: without one, DOS starts the
; program with SS:SP inside its code, and an interrupt there writes into it.
.model medium
.386
.stack 256

extrn _main:far

.data
        db 16384 dup (?)
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
    call far ptr _main
    mov ax, 4c00h
    int 21h
end start
