; x86-code32's C start-up under a DOS extender (DOS/32A): flat, CS = DS = ES = SS. The
; extender enters with ES on the PSP, so the start-up sets ES = DS (string operations use
; it), fills `_STACKLOW` for -fsanitize=stack, then `call _main` and exit with its low
; byte. The extender zero-fills the image's BSS and sets the stack the linker's `.stack`
; names.
.386
.model flat

extrn _main:near

STACK_BYTES equ 16384

.stack STACK_BYTES

; The lowest ESP a checked function may reach: the stack's bottom plus 256 for the handler's
; frames and an interrupt (Open Watcom's `_STACKLOW`).
.data
public _STACKLOW
_STACKLOW dd 0

.code
public start
start:
    push ds
    pop es
    lea eax, [esp - STACK_BYTES + 256]
    mov _STACKLOW, eax
    call _main
    mov ah, 4Ch
    int 21h
end start
