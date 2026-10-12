;; name: start
;; desc: program start-up of x86-m32 under DOS/32A: the stack, the zeroed bss, the hook, main
;;
;; args: the environment the extender gives
;; retn: never returns: LL$EXIT with main's result
;;
;; chng: oct/26 restyled, symbols renamed to LL$ [ali]
;; obs.: The language's start-up hook runs before `main` when its description names one.

;; The start-up under DOS/32A: flat, CS = DS = ES = SS. The extender enters with ES on the PSP, so
;; ES is set to DS; it zero-fills the image's BSS and sets the stack `.stack` names. `LL$STACK_LOW`, the
;; lowest ESP a checked function may reach, is the stack's bottom plus STACK_RESERVE for the panic's frames
;; and an interrupt.
.386
.model flat

extrn _main:near
extrn LL$STACK_LOW:dword
extrn LL$EXIT:near
;; The language's start-up hook, called before `main` when its description names one (`init`).
ifdef LANG_INIT
extrn LANG_INIT:near
endif

;; STACK_BYTES comes from the language's description (nib.toml's stack_base), DOS_* from the OS's facts,
;; STACK_RESERVE from os.toml.

.stack STACK_BYTES

.code
start:
                push    ds
                pop     es
                lea     eax, [esp - STACK_BYTES + STACK_RESERVE]
                mov     LL$STACK_LOW, eax
ifdef LANG_INIT
                call    LANG_INIT
endif
                call    _main
                push    eax
                call    LL$EXIT

end start
