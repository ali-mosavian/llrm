;; name: B$INT10, B$INT17, B$INT21
;; desc: software interrupts
;;
;; args: ax:ptr Regs | the interrupt, with the registers from Regs
;; retn: Regs -> the registers and the flags as the interrupt left them
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed; the QB
;;       runtime's device code (dev*.c, gfxdev.c) is C around these. Target 386+ with an FPU.

                .model  medium, pascal
                .386
                .387
                option  proc:private

                include qb.inc

Regs            struc
                rax             word    ?
                rbx             word    ?
                rcx             word    ?
                rdx             word    ?
                rbp             word    ?
                res             word    ?
                rflags          word    ?
Regs            ends

BlockOp         struc
                dst             dword   ?
                src             dword   ?
                count           word    ?               ;; bytes, or words for B$FILL
                value           word    ?               ;; for B$FILL
BlockOp         ends

.code
;;::::::::::::::
;; software interrupt, with the registers of the Regs at si
CALLINT         macro   vector:req

                pushad                          ;; the BIOS and DOS keep what they like of the registers
                push    es
                mov     si, ax
                push    si
                mov     ax, [si].Regs.rax
                mov     bx, [si].Regs.rbx
                mov     cx, [si].Regs.rcx
                mov     dx, [si].Regs.rdx
                int     vector
                pop     si
                mov     [si].Regs.rax, ax
                mov     [si].Regs.rbx, bx
                mov     [si].Regs.rcx, cx
                mov     [si].Regs.rdx, dx
                mov     [si].Regs.rbp, bp
                mov     ax, es
                mov     [si].Regs.res, ax
                pushf
                pop     W [si].Regs.rflags
                pop     es
                popad
                ret
                endm

;;::::::::::::::
;; B$INT10 (ax: regs)
B$INT10         proc    public

                CALLINT 10h
B$INT10         endp

;;::::::::::::::
;; B$INT17 (ax: regs)
B$INT17         proc    public

                CALLINT 17h
B$INT17         endp

;;::::::::::::::
;; B$INT21 (ax: regs)
B$INT21         proc    public

                CALLINT 21h
B$INT21         endp
                end
