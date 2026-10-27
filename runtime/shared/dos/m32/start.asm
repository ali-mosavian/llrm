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

ifdef MAIN_WATCOM
extrn main_:near
else
extrn _main:near
endif
extrn LL$STACK_LOW:dword
extrn LL$EXIT:near
extrn LL$PSP:word
;; The language's start-up hook, called before `main` when its description names one (`init`).
ifdef LANG_INIT
extrn LANG_INIT:near
endif

;; STACK_BYTES comes from the language's description (nib.toml's stack_base), DOS_* from the OS's facts,
;; STACK_RESERVE from os.toml.

.stack STACK_BYTES

.code
start:
                ;; the extender's stack top is wherever the image ends: aligned, so the frames of a program (and the copies out of them
                ;; whose length depends on an address's low bits) do not move with its size
                and     esp, -16
                mov     LL$PSP, es              ;; the PSP's selector, for the command line and the environment
                push    ds
                pop     es
                lea     eax, [esp - STACK_BYTES + STACK_RESERVE]
                mov     LL$STACK_LOW, eax
ifdef LANG_INIT
                call    LANG_INIT
endif
ifdef MAIN_WATCOM
                call    main_
else
                call    _main
endif
                push    eax
                call    LL$EXIT

end start
