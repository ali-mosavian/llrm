; x86-code32's C start-up under a DOS extender (DOS/32A): flat, CS = DS = ES = SS. The
; extender enters with ES on the PSP, so the start-up sets ES = DS (string operations use
; it), then `call _main` and exit with its low byte. The extender zero-fills the image's
; BSS and sets the stack the linker's `.stack` names.
.386
.model flat

extrn _main:near

.stack 16384

.code
public start
start:
    ; The extender enters with ES on the PSP; string operations need ES = DS, the one flat segment.
    push ds
    pop es
    call _main
    mov ah, 4Ch
    int 21h
end start
