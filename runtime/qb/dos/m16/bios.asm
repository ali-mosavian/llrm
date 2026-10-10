;; name: QB$INT10, QB$INT21
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
                count           word    ?               ;; bytes, or words for QB$FILL
                value           word    ?               ;; for QB$FILL
BlockOp         ends

.code
;;::::::::::::::
;; software interrupt, with the registers of the Regs at si
CALLINT         macro   vector:req

                push    bx
                push    si
                push    bp
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
                pop     bp
                pop     si
                pop     bx
                ret
                endm

;;::::::::::::::
;; QB$INT10 (ax: regs)
QB$INT10         proc    public

                CALLINT 10h
QB$INT10         endp

;;::::::::::::::
;; QB$INT21 (ax: regs)
QB$INT21         proc    public

                CALLINT 21h
QB$INT21         endp
                end
