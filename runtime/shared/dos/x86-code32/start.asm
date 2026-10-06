; The start-up under DOS/32A: flat, CS = DS = ES = SS. The extender enters with ES on the PSP, so
; ES is set to DS; it zero-fills the image's BSS and sets the stack `.stack` names. `_llrm_os_stack_low`, the
; lowest ESP a checked function may reach, is the stack's bottom plus 512 for the panic's frames
; and an interrupt.
.386
.model flat

extrn _main:near
extrn _llrm_os_stack_low:dword
extrn _llrm_os_exit:near
; The language's start-up hook, called before `main` when its description names one (`init`).
ifdef LANG_INIT
extrn LANG_INIT:near
endif

; STACK_BYTES comes from the language's description (nib.toml's stack_base), DOS_* from the OS's facts.
STACK_RESERVE equ 512

.stack STACK_BYTES

.code
start:
    push ds
    pop es
    lea eax, [esp - STACK_BYTES + STACK_RESERVE]
    mov _llrm_os_stack_low, eax
ifdef LANG_INIT
    call LANG_INIT
endif
    call _main
    push eax
    call _llrm_os_exit

end start
