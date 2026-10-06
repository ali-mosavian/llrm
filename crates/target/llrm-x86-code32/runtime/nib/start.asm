; Nib's start-up under DOS/32A: flat, CS = DS = ES = SS. The extender enters with ES on the PSP, so
; ES is set to DS; it zero-fills the image's BSS and sets the stack `.stack` names. `N$OSLO`, the
; lowest ESP a checked function may reach, is the stack's bottom plus 512 for the panic's frames
; and an interrupt.
.386
.model flat

extrn _main:near
extrn N$OSLO:dword

; STACK_BYTES comes from nib.toml (assembler_defines).
STACK_RESERVE equ 512

.stack STACK_BYTES

.code
start:
    push ds
    pop es
    lea eax, [esp - STACK_BYTES + STACK_RESERVE]
    mov N$OSLO, eax
    call _main
    mov ah, 4Ch
    int 21h

end start
