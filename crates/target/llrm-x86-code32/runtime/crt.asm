; x86-code32's C start-up under a DOS extender (DOS/32A): flat, CS = DS = ES = SS, so
; `call _main` and exit with its low byte. The extender zero-fills the image's BSS and
; sets the stack the linker's `.stack` names.
.386
.model flat

extrn _main:near

.stack 16384

.code
public start
start:
    call _main
    mov ah, 4Ch
    int 21h
end start
