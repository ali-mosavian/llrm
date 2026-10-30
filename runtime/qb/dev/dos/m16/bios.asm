;; name: B$INT10, B$INT16, B$INT17, B$INT21
;; desc: software interrupts
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed;
;;       the QB runtime's device code (dev*.c, gfxdev.c) is C around these.
;;       Target 386+ with an FPU.

                .model  medium, pascal
                .386
                .387
                option  proc:private

                include ../../../dos/m16/qb.inc

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
                ;; bytes, or words for B$FILL
                count           word    ?
                value           word    ?               ;; for B$FILL
BlockOp         ends

.code
;; CALLINT - software interrupt, with the registers of the Regs at si.
CALLINT         macro   vector:req

                ;; the BIOS and DOS keep what they like of the registers
                pushad
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

;; B$INT10 - call BIOS or DOS interrupt 10h.
;;
;; Loads ax bx cx dx from `regs`, runs interrupt 10h, and stores ax bx cx dx bp
;; es and the flags back as the interrupt left them.
;;
;; Args:
;;     regs (ax): near ptr to a Regs.
;;
;; Returns:
;;     regs: the registers and the flags as the interrupt left them.
;;
;; Saves:    all general registers and es (pushad, push es)
;; Clobbers: flags
B$INT10         proc    public

                CALLINT 10h
B$INT10         endp

;; B$INT17 - call BIOS or DOS interrupt 17h.
;;
;; Loads ax bx cx dx from `regs`, runs interrupt 17h, and stores ax bx cx dx bp
;; es and the flags back as the interrupt left them.
;;
;; Args:
;;     regs (ax): near ptr to a Regs.
;;
;; Returns:
;;     regs: the registers and the flags as the interrupt left them.
;;
;; Saves:    all general registers and es (pushad, push es)
;; Clobbers: flags
B$INT17         proc    public

                CALLINT 17h
B$INT17         endp

;; B$INT16 - call BIOS or DOS interrupt 16h.
;;
;; Loads ax bx cx dx from `regs`, runs interrupt 16h, and stores ax bx cx dx bp
;; es and the flags back as the interrupt left them.
;;
;; Args:
;;     regs (ax): near ptr to a Regs.
;;
;; Returns:
;;     regs: the registers and the flags as the interrupt left them.
;;
;; Saves:    all general registers and es (pushad, push es)
;; Clobbers: flags
B$INT16         proc    public

                CALLINT 16h
B$INT16         endp

;; B$INT21 - call BIOS or DOS interrupt 21h.
;;
;; Loads ax bx cx dx from `regs`, runs interrupt 21h, and stores ax bx cx dx bp
;; es and the flags back as the interrupt left them.
;;
;; Args:
;;     regs (ax): near ptr to a Regs.
;;
;; Returns:
;;     regs: the registers and the flags as the interrupt left them.
;;
;; Saves:    all general registers and es (pushad, push es)
;; Clobbers: flags
B$INT21         proc    public

                CALLINT 21h
B$INT21         endp
                end
